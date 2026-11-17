//! `@opSweep` and `@opLoft`.

use super::ShapeSet;
use crate::model::{BodyType, Cap, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, field};
use crate::{occt_error, query, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Shape, ShapeKind, Vec3};
use std::rc::Rc;

/// Adds an operation's solids (or, for sheets, shells) with start and end cap infos.
fn add_with_caps(model: &mut Model, id: &FeatureId, op: &occt::Operation, solid: bool) -> R<()> {
    add_with_caps_facing(model, id, op, solid, false)
}

/// As `add_with_caps`, the pieces reversed if `reverse`.
fn add_with_caps_facing(model: &mut Model, id: &FeatureId, op: &occt::Operation, solid: bool, reverse: bool) -> R<()> {
    add_shape_with_caps(model, id, &op.shape(), &op.first(), &op.last(), solid, reverse)
}

/// As [`add_with_caps_facing`], for a shape with its start and end caps given.
fn add_shape_with_caps(
    model: &mut Model,
    id: &FeatureId,
    shape: &Shape,
    first: &[Shape],
    last: &[Shape],
    solid: bool,
    reverse: bool,
) -> R<()> {
    let (mut start_cap, mut end_cap) = (ShapeSet::default(), ShapeSet::default());
    for face in first {
        start_cap.insert_all(face);
    }
    for face in last {
        end_cap.insert_all(face);
    }
    let created = Info::new(id.clone());
    let cap = |c| {
        Rc::new(Info {
            created_by: id.clone(),
            sketch_entity: None,
            cap: Some(c),
            dependency: None,
            ..Default::default()
        })
    };
    let (start_info, end_info) = (cap(Cap::Start), cap(Cap::End));
    let special = |s: &Shape| {
        if end_cap.contains(s) {
            Some(end_info.clone())
        } else if start_cap.contains(s) {
            Some(start_info.clone())
        } else {
            None
        }
    };
    let (kind, body_type) =
        if solid { (ShapeKind::Solid, BodyType::Solid) } else { (ShapeKind::Shell, BodyType::Sheet) };
    let mut pieces = shape.sub_shapes(kind);
    if pieces.is_empty() && !solid {
        pieces = shape.sub_shapes(ShapeKind::Face);
    }
    if pieces.is_empty() {
        return Err(regen_error("SWEEP_FAILED"));
    }
    for piece in pieces {
        let piece = if reverse { piece.reversed() } else { piece };
        model.add_body(NewBody::new(piece, body_type, created.clone()), &Origins::default(), &created, &special);
    }
    Ok(())
}

/// A sweep locked to a surface of revolution (a cylinder, a cone, a sphere or a torus) along a circle round its axis:
/// the profile turns with the surface's normal, a revolve about the axis as far as the path goes (batch 058 c004, a
/// cone's side 070 c007, a frame on a cylinder's rim 070 c009). Along a straight line on a cylinder or a cone (a
/// ruling, along which its normal stays put), the profile is carried along the line (070 c008). `None` when the
/// sweep isn't either.
fn locked_to_revolution(model: &mut Model, id: &FeatureId, definition: &Value) -> R<Option<Value>> {
    use values::{cross, dot, norm, sub};
    if field(definition, "profileControl").as_str() != Some("LOCK_FACES") {
        return Ok(None);
    }
    let lock: Vec<Shape> = query::evaluate(model, field(definition, "lockFaces"))?
        .into_iter()
        .filter(|r| r.kind == EntityType::Face)
        .map(|r| model.shape(r).clone())
        .collect();
    let [face] = lock.as_slice() else { return Ok(None) };
    let Ok(cyl) = face.surface_definition() else { return Ok(None) };
    if !(1..=4).contains(&cyl.kind) {
        return Ok(None);
    }
    // Whole: an arc OCCT split at a cylinder's seam is one arc (batch 138 c005).
    let path: Vec<Shape> = query::evaluate(model, field(definition, "path"))?
        .iter()
        .filter(|r| r.kind == EntityType::Edge)
        .map(|r| super::whole_edge(model, *r))
        .collect();
    let [edge] = path.as_slice() else { return Ok(None) };
    if edge.curve_type().ok() == Some(occt::CurveType::Line) && (cyl.kind == 1 || cyl.kind == 2) {
        return along_ruling(model, id, definition, edge, cyl.frame);
    }
    if edge.curve_type().ok() != Some(occt::CurveType::Circle) {
        return Ok(None);
    }
    let circle = edge.curve_definition().map_err(occt_error)?;
    let (axis, origin) = (cyl.frame.normal, cyl.frame.origin);
    // (A three-point arc's centre may be a few nanometres off, batch 070 c009.) A sphere turns about any axis
    // through its centre: a circle on it whose axis passes there (a great circle, batch 076 c001).
    let about_axis = if cyl.kind == 3 {
        norm(cross(sub(origin, circle.frame.origin), circle.frame.normal)) <= 1e-7
    } else {
        norm(cross(circle.frame.normal, axis)) <= 1e-9 && norm(cross(sub(circle.frame.origin, origin), axis)) <= 1e-7
    };
    if !about_axis {
        return Ok(None);
    }
    let found = query::evaluate(model, field(definition, "profiles"))?;
    let faces: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Face).map(|r| model.shape(*r).clone()).collect();
    if faces.is_empty() {
        return Ok(None);
    }
    // How far round: the whole circle, or the arc between its ends, about the circle's normal turned the way the arc
    // runs from the end the profile is at (std puts it at the far end of an edge a closed frame path runs backwards
    // along, batch 138 c005).
    let ends = edge.edge_end_vertices().map_err(occt_error)?;
    let closed = ends.len() < 2 || ends[0].is_same(&ends[1]);
    let start = edge.edge_tangent(0.0, false).map_err(occt_error)?;
    let end = edge.edge_tangent(1.0, false).map_err(occt_error)?.origin;
    let at_end = faces[0].centroid().is_ok_and(|c| norm(sub(c, end)) < norm(sub(c, start.origin)));
    let (from, to) = if at_end && !closed { (end, start.origin) } else { (start.origin, end) };
    let mut turning = if dot(cross(sub(start.origin, circle.frame.origin), start.direction), circle.frame.normal) < 0.0 {
        values::scale(circle.frame.normal, -1.0)
    } else {
        circle.frame.normal
    };
    if at_end && !closed {
        turning = values::scale(turning, -1.0);
    }
    let angle = if closed {
        2.0 * std::f64::consts::PI
    } else {
        let (u, w) = (sub(from, circle.frame.origin), sub(to, circle.frame.origin));
        let mut t = dot(cross(u, w), turning).atan2(dot(u, w));
        if t <= 0.0 {
            t += 2.0 * std::f64::consts::PI;
        }
        t
    };
    for profile in &faces {
        let op = occt::revolve(profile, occt::Axis { origin: circle.frame.origin, direction: turning }, angle)
            .map_err(|_| regen_error("SWEEP_FAILED"))?;
        add_with_caps(model, id, &op, true)?;
    }
    Ok(Some(Value::UNDEFINED))
}

/// A profile swept along a ruling of a cylinder or a cone it is locked to: carried along the line, each edge (of every
/// loop) making a face Onshape gives as a B-spline, the edge's own across it (linear for a straight edge) and a cubic
/// of evenly spaced poles along the line (batch 070 c008, 183 c004), between the profile and its copy at the far end.
/// `None` if the line isn't a ruling (in a plane through the axis).
fn along_ruling(model: &mut Model, id: &FeatureId, definition: &Value, edge: &Shape, axis: occt::Frame) -> R<Option<Value>> {
    use values::{add, cross, dot, norm, scale, sub};
    let a = edge.edge_tangent(0.0, false).map_err(occt_error)?;
    let b = edge.edge_tangent(1.0, false).map_err(occt_error)?;
    // In a plane through the axis.
    let span = cross(sub(a.origin, axis.origin), axis.normal);
    if norm(span) < 1e-9 || dot(span, a.direction).abs() > 1e-9 * norm(span) {
        return Ok(None);
    }
    let found = query::evaluate(model, field(definition, "profiles"))?;
    let faces: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Face).map(|r| model.shape(*r).clone()).collect();
    let [profile] = faces.as_slice() else { return Ok(None) };
    // Every loop's edges, holes' too (batch 183 c004: a square with a round hole). A profile of straight edges in
    // several loops (a frame's square tube, batch 138 c005) is swept as before: the frame's mitres, cut by replacing
    // its ends, trim its planar sides but not these.
    let loops = profile.sub_shapes(ShapeKind::Wire);
    let profile_edges = profile.sub_shapes(ShapeKind::Edge);
    if loops.len() > 1 && profile_edges.iter().all(|e| e.curve_type().ok() == Some(occt::CurveType::Line)) {
        return Ok(None);
    }
    // From the end of the path the profile is at.
    let (start, end) = {
        let p = profile.face_plane().map_err(occt_error)?;
        if dot(sub(a.origin, p.origin), p.normal).abs() <= dot(sub(b.origin, p.origin), p.normal).abs() {
            (a.origin, b.origin)
        } else {
            (b.origin, a.origin)
        }
    };
    let travel = sub(end, start);
    let mut sides = Vec::new();
    let along = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    for e in &profile_edges {
        if e.curve_type().ok() == Some(occt::CurveType::Line) {
            let ends = e.edge_tangent(0.0, false).map_err(occt_error)?.origin;
            let ende = e.edge_tangent(1.0, false).map_err(occt_error)?.origin;
            let mut poles = Vec::new();
            for p in [ends, ende] {
                for k in 0..4 {
                    poles.push(add(p, scale(travel, k as f64 / 3.0)));
                }
            }
            sides.push(
                occt::bspline_surface_face(&poles, (2, 4), &[0.0, 0.0, 1.0, 1.0], &along, (1, 3)).map_err(occt_error)?,
            );
            continue;
        }
        // A curved edge (a hole's circle): its own B-spline across, as along a straight edge along the line.
        let curve = e.edge_bspline(1e-9, false, false).map_err(occt_error)?;
        let weights = if curve.rational { curve.weights.clone() } else { vec![1.0; curve.poles.len()] };
        let mut poles = Vec::new();
        let mut all_weights = Vec::new();
        for (p, w) in curve.poles.iter().zip(&weights) {
            for k in 0..4 {
                poles.push(add(*p, scale(travel, k as f64 / 3.0)));
                all_weights.push(*w);
            }
        }
        let side = occt::rational_bspline_surface_face(
            &poles,
            &all_weights,
            (curve.poles.len(), 4),
            &curve.knots,
            &along,
            (curve.degree, 3),
        )
        .map_err(occt_error)?;
        sides.push(side);
    }
    let far = occt::transform(profile, &super::translation(travel)).map_err(occt_error)?.shape();
    let mut all = vec![profile.clone()];
    all.extend(sides);
    all.push(far.clone());
    let shell = occt::sew_faces(&all, 1e-7).map_err(occt_error)?;
    let mut solid = occt::solid_from(&shell).map_err(|_| regen_error("SWEEP_FAILED"))?;
    if solid.volume().map_err(occt_error)? < 0.0 {
        solid = solid.reversed();
    }
    // The caps, found again in the sewn solid by where they lie.
    let centre = |f: &Shape| f.centroid().unwrap_or(Vec3 { x: f64::MAX, y: 0.0, z: 0.0 });
    let (start_at, end_at) = (centre(profile), centre(&far));
    let created = Info::new(id.clone());
    let cap = |c| Rc::new(Info { created_by: id.clone(), cap: Some(c), ..Default::default() });
    let (start_info, end_info) = (cap(Cap::Start), cap(Cap::End));
    let special = |f: &Shape| {
        if f.kind() != Some(ShapeKind::Face) {
            return None;
        }
        let c = centre(f);
        if values::norm(sub(c, start_at)) < 1e-7 && f.surface_type().ok() == Some(occt::SurfaceType::Plane) {
            Some(start_info.clone())
        } else if values::norm(sub(c, end_at)) < 1e-7 && f.surface_type().ok() == Some(occt::SurfaceType::Plane) {
            Some(end_info.clone())
        } else {
            None
        }
    };
    model.add_body(NewBody::new(solid, BodyType::Solid, created.clone()), &Origins::default(), &created, &special);
    Ok(Some(Value::UNDEFINED))
}

pub fn sweep(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    if let Some(done) = locked_to_revolution(model, &id, definition)? {
        return Ok(done);
    }
    // Locked to curved faces: the profile turns with their normals along the path (batch 137).
    let mut supported: Option<Vec<Shape>> = None;
    // The profile kept as it is, or turning only about a direction (batch 021).
    let control: Option<Option<Vec3>> = if values::opt_bool(definition, "keepProfileOrientation")?.unwrap_or(false) {
        Some(None)
    } else {
        match field(definition, "profileControl").as_str() {
            None | Some("NONE") | Some("KEEP_ORIENTATION") => None,
            Some("LOCK_DIRECTION") => {
                Some(Some(values::vec3(field(definition, "lockDirection"), "opSweep lockDirection")?))
            }
            // Locked to planar faces sharing a normal: turning only about that normal (a frame on a block's top,
            // batch 030).
            Some("LOCK_FACES") => {
                let faces: Vec<Shape> = query::evaluate(model, field(definition, "lockFaces"))?
                    .into_iter()
                    .filter(|r| r.kind == EntityType::Face)
                    .map(|r| model.shape(r).clone())
                    .collect();
                if faces.is_empty() {
                    return Err(regen_error("SWEEP_SELECT_LOCK_FACES"));
                }
                let normals: Option<Vec<Vec3>> = faces.iter().map(|f| f.face_plane().ok().map(|p| p.normal)).collect();
                match normals {
                    Some(normals) if normals.iter().all(|n| values::norm(values::cross(*n, normals[0])) < 1e-9) => {
                        Some(Some(normals[0]))
                    }
                    _ => {
                        supported = Some(faces);
                        None
                    }
                }
            }
            Some(other) => return Err(error(format!("@opSweep: profileControl {other} isn't supported yet"))),
        }
    };
    let found = query::evaluate(model, field(definition, "profiles"))?;
    let faces: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Face).map(|r| model.shape(*r).clone()).collect();
    let edges: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Edge).map(|r| model.shape(*r).clone()).collect();
    // A closed edge OCCT split at a seam vertex is shown as its first piece: the others go with it (a curve a split
    // left round a cylinder, across its seam, batch 137 c004).
    let mut path_edges: Vec<Shape> = Vec::new();
    for r in query::evaluate(model, field(definition, "path"))?.iter().filter(|r| r.kind == EntityType::Edge) {
        // Running as Onshape's edge does: std places the profile at its start (a frame's segment along a split's
        // edge, batch 138 c005).
        let shown = super::onshape_edge(model, *r);
        let hidden: Vec<Shape> = model.bodies[r.body]
            .shape
            .hidden_edge_pieces()
            .into_iter()
            .filter(|(s, _)| s.is_same(&shown))
            .map(|(_, h)| h)
            .collect();
        for e in std::iter::once(shown).chain(hidden) {
            if !path_edges.iter().any(|p| p.is_same(&e)) {
                path_edges.push(e);
            }
        }
    }
    // Nothing to sweep or nothing to sweep along (a profile sketched on a degenerate plane has no region).
    if path_edges.is_empty() {
        return Err(regen_error("SWEEP_SELECT_PATH"));
    }
    if faces.is_empty() && edges.is_empty() {
        return Err(regen_error("SWEEP_SELECT_PROFILE"));
    }
    let paths = occt::connect_edges(&path_edges).map_err(occt_error)?;
    let [path] = paths.as_slice() else { return Err(regen_error("SWEEP_PATH_NO_CONSTRAINT")) };
    // Locked to faces, a profile can't turn a sharp corner of its path: Onshape finds the path invalid (an L of
    // lines, batch 034), though it mitres such a corner for a profile left free.
    if field(definition, "profileControl").as_str() == Some("LOCK_FACES") && !path.wire_smooth(1e-6) {
        return Err(regen_error("SWEEP_INVALID_PATH"));
    }
    let solid = !faces.is_empty();
    let profiles: Vec<Shape> = if solid {
        // Faces sharing edges merged into one profile; apart, each swept on its own (batch 183 c003: two squares side
        // by side, two solids).
        let mut groups: Vec<Vec<Shape>> = Vec::new();
        for f in &faces {
            let edges = f.sub_shapes(ShapeKind::Edge);
            let touching: Vec<usize> = (0..groups.len())
                .filter(|&g| groups[g].iter().any(|h| h.sub_shapes(ShapeKind::Edge).iter().any(|e| edges.iter().any(|x| x.is_same(e)))))
                .collect();
            let mut merged = vec![f.clone()];
            for &g in touching.iter().rev() {
                merged.extend(groups.remove(g));
            }
            groups.push(merged);
        }
        let mut out = Vec::new();
        for g in groups {
            out.push(if g.len() == 1 { g[0].clone() } else { occt::merge_faces(&g).map_err(occt_error)?.shape() });
        }
        out
    } else {
        occt::connect_edges(&edges).map_err(occt_error)?
    };
    if profiles.is_empty() {
        return Err(regen_error("SWEEP_SELECT_PROFILE"));
    }
    let twist = if values::opt_bool(definition, "hasTwist")?.unwrap_or(false) {
        values::angle(field(definition, "angle"), "opSweep twist angle")?
    } else {
        0.0
    };
    let scale = if values::opt_bool(definition, "hasScale")?.unwrap_or(false) {
        field(definition, "scaleFactor").as_number().ok_or_else(|| error("@opSweep: scaleFactor must be a number"))?
    } else {
        1.0
    };
    if twist.abs() > 1e-12 || (scale - 1.0).abs() > 1e-12 {
        return twisted_or_scaled(model, &id, &profiles, &path_edges, twist, scale, solid);
    }
    for profile in &profiles {
        // A path edge Onshape can lock to curved faces is one of theirs, or a line, circle or ellipse on them: a
        // helix (Onshape's own fit, 7 µm off its cylinder) fails (batch 137 c001–c003).
        if let Some(faces) = &supported {
            for edge in path.sub_shapes(ShapeKind::Edge) {
                let theirs = faces.iter().any(|f| f.sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(&edge)));
                let analytic = matches!(
                    edge.curve_type().ok(),
                    Some(occt::CurveType::Line | occt::CurveType::Circle | occt::CurveType::Ellipse)
                );
                if !theirs && !analytic {
                    return Err(regen_error("SWEEP_FAILED"));
                }
            }
        }
        let op = match (&supported, control) {
            (Some(faces), _) => occt::sweep_supported(profile, path, faces),
            (None, Some(lock)) => occt::sweep_controlled(profile, path, lock),
            (None, None) => occt::sweep(profile, path),
        }
        .map_err(|_| regen_error("SWEEP_FAILED"))?;
        add_with_caps(model, &id, &op, solid)?;
    }
    Ok(Value::UNDEFINED)
}

/// A sweep whose profile turns about the path (by `twist` over its length) and grows (to `scale` times its size),
/// both in proportion to the distance along the whole path (batch 019; along a line and an arc, the twist at their
/// joint is the line's share of the length, batch 057). The path is a chain of lines and circular arcs, each joined
/// smoothly to the next; along each the profile is carried (along a line, moved; round an arc, turned with it about
/// its axis) and lofted through copies, a copy every 2.8° of turn (and of arc, batch 048), ruled between the two ends
/// of a line along which it only grows (a circle makes a cone). The pieces are one face each, sewn end to end.
fn twisted_or_scaled(
    model: &mut Model,
    id: &FeatureId,
    profiles: &[Shape],
    path_edges: &[Shape],
    twist: f64,
    scale: f64,
    solid: bool,
) -> R<Value> {
    use values::{add, cross, dot, norm, normalize, sub};
    let unsupported = || error("@opSweep: twist and scale are only supported along chains of lines and arcs yet");
    // Along a chain with a spline in it Onshape fails (batch 066 c005: a line then a spline, tangent where they
    // meet).
    if path_edges.len() > 1
        && path_edges.iter().any(|e| !matches!(e.curve_type().ok(), Some(occt::CurveType::Line | occt::CurveType::Circle)))
    {
        return Err(regen_error("SWEEP_FAILED"));
    }
    let rot3 = |axis: Vec3, angle: f64| -> [[f64; 3]; 3] {
        let r = super::rotation(occt::Axis { origin: Vec3::new(0.0, 0.0, 0.0), direction: axis }, angle).m;
        [[r[0], r[1], r[2]], [r[4], r[5], r[6]], [r[8], r[9], r[10]]]
    };
    let apply = |m: &[[f64; 3]; 3], v: Vec3| {
        Vec3::new(
            m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z,
            m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z,
            m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z,
        )
    };
    let mul = |a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]| {
        let mut out = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                out[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
            }
        }
        out
    };
    let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    if let [path] = path_edges
        && !matches!(path.curve_type().ok(), Some(occt::CurveType::Line | occt::CurveType::Circle))
    {
        return twisted_along_curve(model, id, profiles, path, twist, scale, solid);
    }
    if path_edges.iter().any(|e| !matches!(e.curve_type().ok(), Some(occt::CurveType::Line | occt::CurveType::Circle)))
    {
        return Err(unsupported());
    }
    // Each edge's ends: point and direction along the edge.
    let mut ends: Vec<[(Vec3, Vec3); 2]> = Vec::new();
    for e in path_edges {
        let a = e.edge_tangent(0.0, false).map_err(occt_error)?;
        let b = e.edge_tangent(1.0, false).map_err(occt_error)?;
        ends.push([(a.origin, a.direction), (b.origin, b.direction)]);
    }
    let close = |a: Vec3, b: Vec3| norm(sub(a, b)) < 1e-7;
    for profile in profiles {
        let wire = if profile.kind() == Some(ShapeKind::Face) {
            occt::outer_wire(profile).map_err(occt_error)?
        } else {
            profile.clone()
        };
        let centre = profile.centroid().map_err(occt_error)?;
        // The chain from the end nearest the profile: each edge with whether it runs backwards.
        let (mut edge, mut back) = (0usize, false);
        let mut best = f64::MAX;
        for (i, e) in ends.iter().enumerate() {
            for (k, end) in e.iter().enumerate() {
                let d = norm(sub(end.0, centre));
                // A free end: no other edge meets it.
                let free =
                    !ends.iter().enumerate().any(|(j, f)| j != i && (close(f[0].0, end.0) || close(f[1].0, end.0)));
                if free && d < best {
                    (best, edge, back) = (d, i, k == 1);
                }
            }
        }
        let mut chain = vec![(edge, back)];
        let mut used = vec![false; ends.len()];
        used[edge] = true;
        loop {
            let (e, b) = *chain.last().expect("chain");
            let tip = ends[e][if b { 0 } else { 1 }].0;
            let Some((next, nb)) = (0..ends.len()).filter(|&j| !used[j]).find_map(|j| {
                if close(ends[j][0].0, tip) {
                    Some((j, false))
                } else if close(ends[j][1].0, tip) {
                    Some((j, true))
                } else {
                    None
                }
            }) else {
                break;
            };
            used[next] = true;
            chain.push((next, nb));
        }
        if chain.len() != path_edges.len() {
            return Err(unsupported());
        }
        // Each piece: its length and the motion carrying a pose at its start a fraction along it.
        type Motion = Box<dyn Fn(f64) -> ([[f64; 3]; 3], Vec3)>;
        let mut pieces: Vec<(f64, Motion, bool, f64)> = Vec::new();
        for &(e, b) in &chain {
            let (start, leaving) = if b { (ends[e][1].0, values::scale(ends[e][1].1, -1.0)) } else { ends[e][0] };
            let other = ends[e][if b { 0 } else { 1 }].0;
            if path_edges[e].curve_type().ok() == Some(occt::CurveType::Circle) {
                let c = path_edges[e].curve_definition().map_err(occt_error)?.frame.origin;
                let axis = normalize(cross(sub(start, c), leaving)).ok_or_else(unsupported)?;
                let (u, w) = (sub(start, c), sub(other, c));
                let mut angle = dot(cross(u, w), axis).atan2(dot(u, w));
                if angle <= 1e-12 {
                    angle += 2.0 * std::f64::consts::PI;
                }
                let motion: Motion = Box::new(move |t: f64| {
                    let r = rot3(axis, angle * t);
                    (r, sub(c, apply(&r, c)))
                });
                pieces.push((norm(u) * angle, motion, true, angle));
            } else {
                let run = sub(other, start);
                let (length, d) = (norm(run), normalize(run).ok_or_else(unsupported)?);
                let motion: Motion = Box::new(move |t: f64| (identity, values::scale(d, length * t)));
                pieces.push((length, motion, false, 0.0));
            }
        }
        let total: f64 = pieces.iter().map(|p| p.0).sum();
        // The path's start, direction there.
        let (e0, b0) = chain[0];
        let (start, leaving) = if b0 { (ends[e0][1].0, values::scale(ends[e0][1].1, -1.0)) } else { ends[e0][0] };
        let step = std::f64::consts::PI / 64.0;
        // Carried so far: x goes to a x + b; the path point and direction there.
        let (mut a, mut b, mut done) = (identity, Vec3::new(0.0, 0.0, 0.0), 0.0);
        let mut groups: Vec<Vec<Shape>> = Vec::new();
        for (index, (length, motion, arc, angle)) in pieces.iter().enumerate() {
            let share = length / total;
            let by_twist = if twist.abs() > 1e-12 { ((twist.abs() * share / step).ceil() as usize).max(8) } else { 1 };
            let by_arc = if *arc { (angle / step).ceil() as usize } else { 1 };
            let steps = by_twist.max(by_arc);
            let mut wires = Vec::new();
            for k in 0..=steps {
                let tau = k as f64 / steps as f64;
                let (r, shift) = motion(tau);
                // The carry to here: first what came before, then this piece's motion (along the first piece, just
                // its motion: the same arithmetic as for a path of one edge, which booleans are sensitive to).
                let (ca, cb) = if index == 0 { (r, shift) } else { (mul(&r, &a), add(apply(&r, b), shift)) };
                let p = add(apply(&ca, start), cb);
                let tangent = apply(&ca, leaving);
                let t = if pieces.len() == 1 { tau } else { (done + length * tau) / total };
                // A positive (counterclockwise) twist turns right-handed about the path's direction (batch 019).
                let turn = rot3(tangent, twist * t);
                let s = 1.0 + (scale - 1.0) * t;
                // x goes to p + s turn (ca x + cb - p).
                let m3 = mul(&turn, &ca);
                let off = add(p, values::scale(apply(&turn, sub(cb, p)), s));
                let m = [
                    s * m3[0][0],
                    s * m3[0][1],
                    s * m3[0][2],
                    off.x,
                    s * m3[1][0],
                    s * m3[1][1],
                    s * m3[1][2],
                    off.y,
                    s * m3[2][0],
                    s * m3[2][1],
                    s * m3[2][2],
                    off.z,
                ];
                wires.push(occt::transform(&wire, &occt::Transform { m }).map_err(occt_error)?.shape());
            }
            let (r, shift) = motion(1.0);
            (a, b) = (mul(&r, &a), add(apply(&r, b), shift));
            done += length;
            groups.push(wires);
        }
        let op = if groups.len() == 1 {
            occt::loft(&groups[0], solid).map_err(|_| regen_error("SWEEP_FAILED"))?
        } else {
            occt::loft_pieces(&groups, solid).map_err(|_| regen_error("SWEEP_FAILED"))?
        };
        add_with_caps(model, id, &op, solid)?;
    }
    Ok(Value::UNDEFINED)
}

/// A sweep twisted or scaled along one curve other than a line or arc (a spline, batch 065): the profile carried
/// along it by the rotation-minimising frame (double reflection; round a planar curve, turned about its plane's
/// normal as round an arc), turned about the path by `twist` and grown to `scale` in proportion to the distance
/// along it, and lofted through copies spaced evenly along the path, one every 2.8° of twist or of turn.
fn twisted_along_curve(
    model: &mut Model,
    id: &FeatureId,
    profiles: &[Shape],
    path: &Shape,
    twist: f64,
    scale: f64,
    solid: bool,
) -> R<Value> {
    use values::{cross, dot, norm, normalize, sub};
    let failed = || regen_error("SWEEP_FAILED");
    let rot3 = |axis: Vec3, angle: f64| -> [[f64; 3]; 3] {
        let r = super::rotation(occt::Axis { origin: Vec3::new(0.0, 0.0, 0.0), direction: axis }, angle).m;
        [[r[0], r[1], r[2]], [r[4], r[5], r[6]], [r[8], r[9], r[10]]]
    };
    let apply = |m: &[[f64; 3]; 3], v: Vec3| {
        Vec3::new(
            m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z,
            m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z,
            m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z,
        )
    };
    let mul = |a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]| {
        let mut out = [[0.0; 3]; 3];
        for (i, row) in out.iter_mut().enumerate() {
            for (j, x) in row.iter_mut().enumerate() {
                *x = (0..3).map(|k| a[i][k] * b[k][j]).sum();
            }
        }
        out
    };
    // The frame's columns as a matrix taking the start frame to it.
    let frame_matrix = |t: Vec3, r: Vec3| {
        let s = cross(t, r);
        [[t.x, r.x, s.x], [t.y, r.y, s.y], [t.z, r.z, s.z]]
    };
    let transpose = |m: [[f64; 3]; 3]| [[m[0][0], m[1][0], m[2][0]], [m[0][1], m[1][1], m[2][1]], [m[0][2], m[1][2], m[2][2]]];
    for profile in profiles {
        let wire = if profile.kind() == Some(ShapeKind::Face) {
            occt::outer_wire(profile).map_err(occt_error)?
        } else {
            profile.clone()
        };
        let centre = profile.centroid().map_err(occt_error)?;
        let (a, b) = (path.edge_tangent(0.0, true).map_err(occt_error)?, path.edge_tangent(1.0, true).map_err(occt_error)?);
        // From the end nearest the profile.
        let back = norm(sub(b.origin, centre)) < norm(sub(a.origin, centre));
        let sample = |t: f64| -> R<(Vec3, Vec3)> {
            let ax = path.edge_tangent(if back { 1.0 - t } else { t }, false).map_err(occt_error)?;
            Ok((ax.origin, if back { values::scale(ax.direction, -1.0) } else { ax.direction }))
        };
        // How far the tangent turns, to space the copies.
        let mut turning = 0.0;
        let mut previous = sample(0.0)?.1;
        for k in 1..=256 {
            let t = sample(k as f64 / 256.0)?.1;
            turning += dot(previous, t).clamp(-1.0, 1.0).acos();
            previous = t;
        }
        let step = std::f64::consts::PI / 64.0;
        let steps = (((twist.abs().max(turning)) / step).ceil() as usize).max(16);
        let (p0, t0) = sample(0.0)?;
        let r0 = normalize(cross(t0, cross(Vec3::new(0.0, 0.0, 1.0), t0)))
            .or_else(|| normalize(cross(t0, cross(Vec3::new(1.0, 0.0, 0.0), t0))))
            .ok_or_else(failed)?;
        let start = transpose(frame_matrix(t0, r0));
        let (mut x, mut t, mut r) = (p0, t0, r0);
        let mut wires = Vec::new();
        for k in 0..=steps {
            let f = k as f64 / steps as f64;
            if k > 0 {
                // Double reflection (Wang, Jüttler, Zheng and Liu 2008).
                let (x1, t1) = sample(f)?;
                let v1 = sub(x1, x);
                let c1 = dot(v1, v1);
                let rl = sub(r, values::scale(v1, 2.0 * dot(v1, r) / c1));
                let tl = sub(t, values::scale(v1, 2.0 * dot(v1, t) / c1));
                let v2 = sub(t1, tl);
                let c2 = dot(v2, v2);
                r = if c2 > 1e-24 { sub(rl, values::scale(v2, 2.0 * dot(v2, rl) / c2)) } else { rl };
                r = normalize(sub(r, values::scale(t1, dot(r, t1)))).ok_or_else(failed)?;
                (x, t) = (x1, t1);
            }
            let carry = mul(&frame_matrix(t, r), &start);
            let turn = rot3(t, twist * f);
            let s = 1.0 + (scale - 1.0) * f;
            // A point y goes to x + s turn carry (y − p0).
            let m3 = mul(&turn, &carry);
            let off = sub(x, values::scale(apply(&m3, p0), s));
            let m = [
                s * m3[0][0], s * m3[0][1], s * m3[0][2], off.x,
                s * m3[1][0], s * m3[1][1], s * m3[1][2], off.y,
                s * m3[2][0], s * m3[2][1], s * m3[2][2], off.z,
            ];
            wires.push(occt::transform(&wire, &occt::Transform { m }).map_err(occt_error)?.shape());
        }
        let op = occt::loft(&wires, solid).map_err(|_| failed())?;
        add_with_caps(model, id, &op, solid)?;
    }
    Ok(Value::UNDEFINED)
}

/// A circle lofted to a profile with corners is split into as many arcs as there are corners, each as long as the
/// others, from where the ray through the corner first round anticlockwise from its start meets it: one arc to each
/// edge, as Onshape splits it (batch 004 c039: a square's corners at 45°, 135°, 225° and 315°; batch 170: a 20 × 10
/// rectangle's from 26.57°).
fn match_circles_to_corners(wires: &mut [Shape]) -> R<()> {
    use values::{cross, dot, sub};
    let corners: Vec<Vec<Vec3>> = wires
        .iter()
        .map(|w| {
            if w.kind() == Some(ShapeKind::Vertex) {
                return Vec::new();
            }
            w.sub_shapes(ShapeKind::Vertex).iter().filter_map(|v| v.vertex_point().ok()).collect()
        })
        .collect();
    let Some(cornered) = corners.iter().position(|c| c.len() >= 2) else { return Ok(()) };
    for i in 0..wires.len() {
        let edges = wires[i].sub_shapes(ShapeKind::Edge);
        let [edge] = edges.as_slice() else { continue };
        if edge.curve_type().ok() != Some(occt::CurveType::Circle) || corners[i].len() > 1 {
            continue;
        }
        let conic = edge.curve_definition().map_err(occt_error)?;
        let (frame, radius) = (conic.frame, conic.radius1);
        let y = cross(frame.normal, frame.x);
        let mut angles: Vec<f64> = corners[cornered]
            .iter()
            .map(|p| {
                let d = sub(*p, frame.origin);
                dot(d, y).atan2(dot(d, frame.x)).rem_euclid(2.0 * std::f64::consts::PI)
            })
            .collect();
        angles.sort_by(f64::total_cmp);
        angles.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        if angles.len() < 2 {
            continue;
        }
        // In arcs as long as each other, from the corner first round anticlockwise from the circle's start: a 20 × 10
        // rectangle's sides each take a quarter of it, from 26.57° (batch 170); a square's corners are at its quarters
        // anyway (batch 004 c039).
        let n = angles.len();
        let first = angles[0];
        angles = (0..n).map(|k| first + std::f64::consts::TAU * k as f64 / n as f64).collect();
        let mut arcs = Vec::new();
        for (k, &start) in angles.iter().enumerate() {
            let end = if k + 1 < angles.len() { angles[k + 1] } else { angles[0] + 2.0 * std::f64::consts::PI };
            arcs.push(occt::arc_edge(frame, radius, start, end).map_err(occt_error)?);
        }
        let mut connected = occt::connect_edges(&arcs).map_err(occt_error)?;
        if connected.len() == 1 {
            wires[i] = connected.remove(0);
        }
    }
    Ok(())
}

/// Onshape's correspondence along a side of a loft between profiles joined by connections (batch 182 c004, c006):
/// the side's parameter u runs from 0 to 1, each point where the profiles are joined at the mean of the fractions of
/// the side the two profiles have come there; each profile's fraction a cubic in u from one such point to the next
/// (Hermite), its slope at each the geometric mean of the stretches' either side (their fractions over their u), at
/// an end of the side its stretch's own (round a whole loop, the last's and the first's).
struct Correspondence {
    nodes: Vec<f64>,
    /// Each profile's fractions at the nodes and its slopes there.
    profiles: [(Vec<f64>, Vec<f64>); 2],
}

impl Correspondence {
    fn new(fractions: [&[f64]; 2], periodic: bool) -> Option<Self> {
        let m = fractions[0].len();
        if m < 2 || fractions[1].len() != m {
            return None;
        }
        let nodes: Vec<f64> = (0..m).map(|g| (fractions[0][g] + fractions[1][g]) / 2.0).collect();
        if nodes.windows(2).any(|w| w[1] <= w[0]) {
            return None;
        }
        let profile = |f: &[f64]| -> Option<(Vec<f64>, Vec<f64>)> {
            let secant: Vec<f64> = (0..m - 1).map(|g| (f[g + 1] - f[g]) / (nodes[g + 1] - nodes[g])).collect();
            if secant.iter().any(|&s| s <= 0.0) {
                return None;
            }
            let mut slopes = vec![0.0; m];
            for g in 1..m - 1 {
                slopes[g] = (secant[g - 1] * secant[g]).sqrt();
            }
            if periodic {
                let s = (secant[m - 2] * secant[0]).sqrt();
                (slopes[0], slopes[m - 1]) = (s, s);
            } else {
                (slopes[0], slopes[m - 1]) = (secant[0], secant[m - 2]);
            }
            Some((f.to_vec(), slopes))
        };
        let profiles = [profile(fractions[0])?, profile(fractions[1])?];
        Some(Self { nodes, profiles })
    }

    /// Profile `p`'s fraction at u and its slope.
    fn at(&self, p: usize, u: f64) -> (f64, f64) {
        let last = self.nodes.len() - 2;
        let g = (0..last).find(|&g| u <= self.nodes[g + 1]).unwrap_or(last);
        let (u0, h) = (self.nodes[g], self.nodes[g + 1] - self.nodes[g]);
        let t = (u - u0) / h;
        let (values, slopes) = &self.profiles[p];
        let (p0, p1, m0, m1) = (values[g], values[g + 1], slopes[g] * h, slopes[g + 1] * h);
        let (t2, t3) = (t * t, t * t * t);
        let value =
            (2.0 * t3 - 3.0 * t2 + 1.0) * p0 + (t3 - 2.0 * t2 + t) * m0 + (3.0 * t2 - 2.0 * t3) * p1 + (t3 - t2) * m1;
        let slope =
            ((6.0 * t2 - 6.0 * t) * (p0 - p1) + (3.0 * t2 - 4.0 * t + 1.0) * m0 + (3.0 * t2 - 2.0 * t) * m1) / h;
        (value, slope)
    }
}

/// How far a cubic Hermite span from `u0` to `u1` of a curve (its point and slope at u) strays at its middle from the
/// curve's own point there.
fn midpoint_off(curve: &dyn Fn(f64) -> (Vec3, Vec3), u0: f64, u1: f64) -> f64 {
    use values::{add, norm, scale, sub};
    let ((p0, d0), (p1, d1), (pm, _)) = (curve(u0), curve(u1), curve((u0 + u1) / 2.0));
    let cubic = add(scale(add(p0, p1), 0.5), scale(sub(d0, d1), (u1 - u0) / 8.0));
    norm(sub(cubic, pm))
}

/// Onshape's spans from one connection (or corner) to the next (batches 180–182, 186): each halved until its cubic
/// strays at its middle within a tolerance of the curve; `off` how far the span from one u to another strays. Pushes
/// the spans' ends after `u0`.
fn halved_spans(u0: f64, u1: f64, off: &dyn Fn(f64, f64) -> f64, tolerance: f64, ends: &mut Vec<f64>, depth: usize) {
    if depth < 10 && off(u0, u1) > tolerance {
        let middle = (u0 + u1) / 2.0;
        halved_spans(u0, middle, off, tolerance, ends, depth + 1);
        halved_spans(middle, u1, off, tolerance, ends, depth + 1);
    } else {
        ends.push(u1);
    }
}

/// The tolerance Onshape halves the spans of a loft with connections to: 1 µm with one connection (batch 186 c003:
/// spans 0.999 µm off kept, 1.15 halved; 187 c010: 1.06 halved; batch 180's 22.5° spans of a circle, 0.62 µm, kept),
/// 0.11 µm with more (batch 182 c006: 0.107 µm kept, 0.113 halved).
fn connected_span_tolerance(connections: usize) -> f64 {
    if connections > 1 { 1.1e-7 } else { 1e-6 }
}

/// A clamped C1 cubic through stations (u, point, slope): each point at a double knot, the poles either side a third
/// of the span along the slope. Its poles and knots.
fn hermite_poles(stations: &[(f64, Vec3, Vec3)]) -> (Vec<Vec3>, Vec<f64>) {
    use values::{add, scale, sub};
    let n = stations.len() - 1;
    let mut poles = vec![stations[0].1];
    let mut knots = vec![stations[0].0; 4];
    for k in 0..=n {
        let (u, p, d) = stations[k];
        if k > 0 {
            poles.push(sub(p, scale(d, (u - stations[k - 1].0) / 3.0)));
        }
        if k < n {
            poles.push(add(p, scale(d, (stations[k + 1].0 - u) / 3.0)));
        }
        if k > 0 && k < n {
            knots.extend([u, u]);
        }
    }
    poles.push(stations[n].1);
    knots.extend([stations[n].0; 4]);
    (poles, knots)
}

/// The edge along stations' C1 cubic ([`hermite_poles`]) from u0 to u1, both stations'.
fn stations_edge(stations: &[(f64, Vec3, Vec3)], u0: f64, u1: f64) -> R<Shape> {
    let within: Vec<(f64, Vec3, Vec3)> =
        stations.iter().copied().filter(|s| s.0 >= u0 - 1e-12 && s.0 <= u1 + 1e-12).collect();
    let (poles, knots) = hermite_poles(&within);
    occt::bspline_edge(&poles, &[], &knots, 3, false).map_err(occt_error)
}

/// A polygon lofted to a circle with connections, a point of each (a corner or a point along an edge of the polygon,
/// a point round the circle), as Onshape lofts it (batches 180–182): round from each connection to the next, the
/// polygon's corners between (and the connection points along edges) divide the circle's arc between them into as
/// many equal arcs, each taken to the next in turn (one connection: the whole circle from it); each side ruled
/// between its edge and its arcs, both cubic Hermite spans along the [`Correspondence`] of the edge and its arcs
/// (halved till close: [`halved_spans`]), and the circle's end
/// bounded by those arcs, a vertex at each point. The solid and its caps (first and last profile), or `None` for
/// other profiles.
fn polygon_circle_connected(
    wires: &[Shape],
    polygon: usize,
    circle: &occt::Conic,
    pairs: &[(Vec3, Vec3)],
    solid: bool,
) -> R<Option<(Shape, Vec<Shape>, Vec<Shape>)>> {
    use values::{add, cross, dot, norm, normalize, scale, sub};
    if !solid || pairs.is_empty() {
        return Ok(None);
    }
    let Some(mut corners) = polygon_corners(&wires[polygon]) else { return Ok(None) };
    let centroid = scale(corners.iter().fold(Vec3::new(0.0, 0.0, 0.0), |a, &c| add(a, c)), 1.0 / corners.len() as f64);
    let centre = circle.frame.origin;
    // Round the way from the first profile to the second, anticlockwise (whichever the circle is); with three
    // connections or more, the way the polygon's first edge was drawn (batches 186 c004, 187 c005–c007: a rectangle's,
    // up its left side, clockwise; 182 c005, 187 c004: polygons of lines drawn anticlockwise, anticlockwise).
    let travel = if polygon == 0 { sub(centre, centroid) } else { sub(centroid, centre) };
    let Some(mut axis) = normalize(travel) else { return Ok(None) };
    if pairs.len() >= 3 {
        let first = &wires[polygon].sub_shapes(ShapeKind::Edge)[0];
        let start = first.edge_tangent(0.0, false).map_err(occt_error)?;
        let drawn = if first.is_reversed() { scale(start.direction, -1.0) } else { start.direction };
        if dot(cross(sub(start.origin, centroid), drawn), axis) < 0.0 {
            axis = scale(axis, -1.0);
        }
    }
    if dot(cross(sub(corners[0], centroid), sub(corners[1], centroid)), axis) < 0.0 {
        corners.reverse();
    }
    let close = |a: Vec3, b: Vec3| norm(sub(a, b)) < 1e-7;
    let n = corners.len();
    // The polygon's points round from its first connection: its corners, and connections along edges; each with the
    // circle's point where it's a connection's.
    let on_edge = |p: Vec3, k: usize| {
        let (a, b) = (corners[k], corners[(k + 1) % n]);
        let (d, e) = (sub(p, a), sub(b, a));
        norm(cross(d, e)) < 1e-7 * norm(e) && !close(p, a) && !close(p, b) && dot(d, e) > 0.0 && dot(d, e) < dot(e, e)
    };
    let mut points: Vec<(Vec3, bool, Option<Vec3>)> = Vec::new();
    for k in 0..n {
        let at_corner = pairs.iter().find(|(p, _)| close(*p, corners[k])).map(|&(_, c)| c);
        points.push((corners[k], true, at_corner));
        let mut along: Vec<(f64, Vec3, Vec3)> = pairs
            .iter()
            .filter(|(p, _)| on_edge(*p, k))
            .map(|&(p, c)| (norm(sub(p, corners[k])), p, c))
            .collect();
        along.sort_by(|a, b| a.0.total_cmp(&b.0));
        points.extend(along.into_iter().map(|(_, p, c)| (p, false, Some(c))));
    }
    let Some(first) = points.iter().position(|p| p.2.is_some()) else { return Ok(None) };
    points.rotate_left(first);
    let count = points.len();
    let radius = circle.radius1;
    let Some(x) = normalize(sub(points[0].2.unwrap_or(centre), centre)) else { return Ok(None) };
    let y = cross(axis, x);
    let tau = std::f64::consts::TAU;
    let angle_of = |c: Vec3| {
        let d = sub(c, centre);
        dot(d, y).atan2(dot(d, x)).rem_euclid(tau)
    };
    // Each point's angle round the circle: the connections' own, those between shared out evenly.
    let marks: Vec<usize> = (0..count).filter(|&i| points[i].2.is_some()).collect();
    let mut angles = vec![0.0; count];
    for (m, &i) in marks.iter().enumerate() {
        let j = marks.get(m + 1).copied().unwrap_or(count);
        let a = angle_of(points[i].2.unwrap_or(centre));
        let mut b = if j < count { angle_of(points[j].2.unwrap_or(centre)) } else { tau };
        if j < count && b <= a {
            b += tau;
        }
        if j == count && marks.len() > 1 {
            b = tau;
        }
        for k in i..j {
            angles[k] = a + (b - a) * (k - i) as f64 / (j - i) as f64;
        }
    }
    let tolerance = connected_span_tolerance(pairs.len());
    let on_circle = |theta: f64| add(centre, add(scale(x, radius * theta.cos()), scale(y, radius * theta.sin())));
    let tangent = |theta: f64| add(scale(x, -radius * theta.sin()), scale(y, radius * theta.cos()));
    let mut sides = Vec::new();
    let mut arc_edges = Vec::new();
    let mut i = points.iter().position(|p| p.1).unwrap_or(0);
    for _ in 0..n {
        let mut j = (i + 1) % count;
        while !points[j].1 {
            j = (j + 1) % count;
        }
        let segments = (j + count - i) % count;
        let (a, b) = (points[i].0, points[j].0);
        // The points' angles round the circle, rising, and their fractions of the edge and of its arcs.
        let mut thetas: Vec<f64> =
            (0..=segments).map(|g| angles[(i + g) % count] + if i + g >= count { tau } else { 0.0 }).collect();
        for g in 1..=segments {
            while thetas[g] <= thetas[g - 1] {
                thetas[g] += tau;
            }
        }
        let sweep = thetas[segments] - thetas[0];
        let length = norm(sub(b, a));
        let mut edge_f: Vec<f64> = (0..=segments).map(|g| norm(sub(points[(i + g) % count].0, a)) / length).collect();
        edge_f[segments] = 1.0;
        let arc_f: Vec<f64> = thetas.iter().map(|t| (t - thetas[0]) / sweep).collect();
        let Some(along) = Correspondence::new([&edge_f, &arc_f], false) else { return Ok(None) };
        let edge_at = |u: f64| {
            let (f, s) = along.at(0, u);
            (add(a, scale(sub(b, a), f)), scale(sub(b, a), s))
        };
        let arc_at = |u: f64| {
            let (f, s) = along.at(1, u);
            let theta = thetas[0] + sweep * f;
            (on_circle(theta), scale(tangent(theta), sweep * s))
        };
        let mut breaks = vec![0.0];
        for g in 0..segments {
            let (u0, u1) = (along.nodes[g], along.nodes[g + 1]);
            halved_spans(u0, u1, &|p, q| midpoint_off(&arc_at, p, q), tolerance, &mut breaks, 0);
        }
        let edge_stations: Vec<(f64, Vec3, Vec3)> = breaks.iter().map(|&u| (u, edge_at(u).0, edge_at(u).1)).collect();
        let arc_stations: Vec<(f64, Vec3, Vec3)> = breaks.iter().map(|&u| (u, arc_at(u).0, arc_at(u).1)).collect();
        let (edge_row, knots) = hermite_poles(&edge_stations);
        let (arc_row, _) = hermite_poles(&arc_stations);
        // v from the first profile to the second.
        let rows = if polygon == 0 { [&edge_row, &arc_row] } else { [&arc_row, &edge_row] };
        let poles: Vec<Vec3> = (0..edge_row.len()).flat_map(|u| [rows[0][u], rows[1][u]]).collect();
        let side = occt::bspline_surface_face(&poles, (edge_row.len(), 2), &knots, &[0.0, 0.0, 1.0, 1.0], (3, 1))
            .map_err(|_| regen_error("LOFT_FAILED"))?;
        // Its boundary along the circle in as many edges as it takes segments.
        let side = if segments > 1 {
            let c = if polygon == 0 { 1.0 } else { 0.0 };
            let e = 1.0 - c;
            let mut loop_uv: Vec<Vec<(f64, f64)>> = vec![vec![(0.0, e), (1.0, e)], vec![(1.0, e), (1.0, c)]];
            for g in (0..segments).rev() {
                loop_uv.push(vec![(along.nodes[g + 1], c), (along.nodes[g], c)]);
            }
            loop_uv.push(vec![(0.0, c), (0.0, e)]);
            occt::face_from_uv(&side, &loop_uv).map_err(|_| regen_error("LOFT_FAILED"))?
        } else {
            side
        };
        sides.push(side);
        // The circle's edges end at each of the points: each a segment's spans.
        for g in 0..segments {
            arc_edges.push(stations_edge(&arc_stations, along.nodes[g], along.nodes[g + 1])?);
        }
        i = j;
    }
    let polygon_cap = occt::fill_edges(&wires[polygon].sub_shapes(ShapeKind::Edge)).map_err(|_| regen_error("LOFT_FAILED"))?;
    let circle_cap = occt::fill_edges(&arc_edges).map_err(|_| regen_error("LOFT_FAILED"))?;
    let mut faces = sides.clone();
    faces.push(polygon_cap.clone());
    faces.push(circle_cap.clone());
    let sewn = occt::sew_faces(&faces, 1e-6).map_err(|_| regen_error("LOFT_FAILED"))?;
    let shape = occt::solid_from(&sewn).map_err(|_| regen_error("LOFT_FAILED"))?;
    let shape = if shape.volume().map_err(occt_error)? < 0.0 { shape.reversed() } else { shape };
    // The caps, found again: sewing makes new faces of them.
    let find = |c: &Shape| -> Vec<Shape> {
        let Ok(p) = c.face_inner_point() else { return Vec::new() };
        shape
            .sub_shapes(ShapeKind::Face)
            .into_iter()
            .filter(|f| f.surface_type().ok() == Some(occt::SurfaceType::Plane))
            .filter(|f| occt::vertex(p).ok().and_then(|v| v.distance(f).ok()).is_some_and(|d| d.distance < 1e-7))
            .collect()
    };
    let (first, last) = if polygon == 0 { (find(&polygon_cap), find(&circle_cap)) } else { (find(&circle_cap), find(&polygon_cap)) };
    Ok(Some((shape, first, last)))
}

/// Two circles joined by more than one connection, as Onshape lofts them (batches 182 c004, 186 c001, c002, 187 c008,
/// c009): one side all the way round, periodic, from the first connection of two or the last of more, its u the
/// circles' [`Correspondence`] round from there (each circle's fraction its angle's, running as its own circle does),
/// cubic Hermite spans halved till close ([`halved_spans`]); the ends bounded by the side's edges, a vertex at each
/// connection. Each circle: its frame (x towards the first connection, its normal the way it runs) and radius, and the
/// connections' angles round it from there, rising. The solid and its caps (first and last profile).
fn circles_connected(
    circles: [(occt::Frame, f64); 2],
    angles: [&[f64]; 2],
    solid: bool,
) -> R<Option<(Shape, Vec<Shape>, Vec<Shape>)>> {
    use values::{add, cross, dot, scale, sub};
    if !solid {
        return Ok(None);
    }
    let tau = std::f64::consts::TAU;
    let m = angles[0].len();
    // From the first connection when there are two (batches 182 c004, 187 c009), from the last round when there are
    // more (186 c002, the third of three; 187 c008, the fourth of four).
    let start = if m > 2 { m - 1 } else { 0 };
    let circles: Vec<(occt::Frame, f64)> = (0..2)
        .map(|c| {
            let (frame, radius) = &circles[c];
            let (a, y) = (angles[c][start], cross(frame.normal, frame.x));
            let x = add(scale(frame.x, a.cos()), scale(y, a.sin()));
            (occt::Frame { x, ..frame.clone() }, *radius)
        })
        .collect();
    let fractions = |c: usize| -> Vec<f64> {
        (0..m).map(|j| (angles[c][(start + j) % m] - angles[c][start]).rem_euclid(tau) / tau).chain([1.0]).collect()
    };
    let (f0, f1) = (fractions(0), fractions(1));
    let Some(round) = Correspondence::new([&f0, &f1], true) else { return Ok(None) };
    let at = |c: usize, u: f64| -> (Vec3, Vec3) {
        let (frame, radius) = &circles[c];
        let y = cross(frame.normal, frame.x);
        let (f, s) = round.at(c, u);
        let theta = tau * f;
        let point = add(frame.origin, add(scale(frame.x, radius * theta.cos()), scale(y, radius * theta.sin())));
        let slope = add(scale(frame.x, -radius * theta.sin()), scale(y, radius * theta.cos()));
        (point, scale(slope, tau * s))
    };
    let mut breaks = vec![0.0];
    for g in 0..round.nodes.len() - 1 {
        let off = |p: f64, q: f64| midpoint_off(&|u| at(0, u), p, q).max(midpoint_off(&|u| at(1, u), p, q));
        halved_spans(round.nodes[g], round.nodes[g + 1], &off, connected_span_tolerance(m), &mut breaks, 0);
    }
    let stations: Vec<Vec<(f64, Vec3, Vec3)>> =
        (0..2).map(|c| breaks.iter().map(|&u| (u, at(c, u).0, at(c, u).1)).collect()).collect();
    let (row0, knots) = hermite_poles(&stations[0]);
    let (row1, _) = hermite_poles(&stations[1]);
    let poles: Vec<Vec3> = row0.iter().zip(&row1).flat_map(|(&a, &b)| [a, b]).collect();
    let side = occt::u_periodic_bspline_surface_face(&poles, (row0.len(), 2), &knots, &[0.0, 0.0, 1.0, 1.0], (3, 1))
        .map_err(|_| regen_error("LOFT_FAILED"))?;
    // Its edges round each end split at the connections. Its seam (OCCT's; Onshape's face has none) away from them,
    // midway along the first span, where the engine shows the pieces either side as one edge: at a connection it
    // would hide the vertex there.
    let nodes = &round.nodes;
    let seam = breaks[1] / 2.0;
    let mut ends: Vec<f64> = vec![seam];
    ends.extend(nodes[1..].iter().copied());
    ends.push(1.0 + seam);
    let mut loop_uv: Vec<Vec<(f64, f64)>> = ends.windows(2).map(|w| vec![(w[0], 0.0), (w[1], 0.0)]).collect();
    loop_uv.push(vec![(1.0 + seam, 0.0), (1.0 + seam, 1.0)]);
    loop_uv.extend(ends.windows(2).rev().map(|w| vec![(w[1], 1.0), (w[0], 1.0)]));
    loop_uv.push(vec![(seam, 1.0), (seam, 0.0)]);
    let side = occt::face_from_uv(&side, &loop_uv).map_err(|_| regen_error("LOFT_FAILED"))?;
    // The ends bounded by the side's own edges.
    let mut caps = Vec::new();
    for (frame, _) in &circles {
        let edges: Vec<Shape> = side
            .sub_shapes(ShapeKind::Edge)
            .into_iter()
            .filter(|e| {
                e.edge_tangent(0.5, false).is_ok_and(|a| dot(sub(a.origin, frame.origin), frame.normal).abs() < 1e-9)
            })
            .collect();
        caps.push(occt::fill_edges(&edges).map_err(|_| regen_error("LOFT_FAILED"))?);
    }
    let sewn =
        occt::sew_faces(&[side, caps[0].clone(), caps[1].clone()], 1e-6).map_err(|_| regen_error("LOFT_FAILED"))?;
    let shape = occt::solid_from(&sewn).map_err(|_| regen_error("LOFT_FAILED"))?;
    let shape = if shape.volume().map_err(occt_error)? < 0.0 { shape.reversed() } else { shape };
    // The caps, found again: sewing makes new faces of them.
    let find = |c: &Shape| -> Vec<Shape> {
        let Ok(p) = c.face_inner_point() else { return Vec::new() };
        shape
            .sub_shapes(ShapeKind::Face)
            .into_iter()
            .filter(|f| f.surface_type().ok() == Some(occt::SurfaceType::Plane))
            .filter(|f| occt::vertex(p).ok().and_then(|v| v.distance(f).ok()).is_some_and(|d| d.distance < 1e-7))
            .collect()
    };
    Ok(Some((shape.clone(), find(&caps[0]), find(&caps[1]))))
}

/// A closed profile of smooth pieces (lines, arcs and ellipses meeting tangent, not all of one circle) lofted to a
/// circle, as Onshape lofts them (batch 187 c010–c013): one periodic side of cubic Hermite spans along the profiles'
/// [`Correspondence`]. Nodes at the profile's vertices and connection points, by arc length round it (within each
/// piece its own parameter, an ellipse's angle); the circle shared evenly among them from the start (between
/// connections, as a polygon's corners share it); the start the first edge's drawn start and the circle's point in
/// its direction, or a connection (of two the first, of more the last); spans halved till within 10 nm, or as
/// connected lofts' ([`connected_span_tolerance`]). The side's edges split at the profile's vertices, and with
/// connections at theirs and the circle's points too (unless one connection is all a loop without vertices has);
/// the ends bounded by them. `connections`: each one's point on the profile (its edge and parameter there) and on
/// the circle. The solid and its caps (first and last profile), or `None` for other profiles.
fn smooth_loft_to_circle(
    wires: &[Shape],
    circle_at: usize,
    connections: &[((Shape, f64), Vec3)],
    solid: bool,
) -> R<Option<(Shape, Vec<Shape>, Vec<Shape>)>> {
    use values::{add, cross, dot, norm, normalize, scale, sub};
    if wires.len() != 2 || !solid {
        return Ok(None);
    }
    let tau = std::f64::consts::TAU;
    let profile = &wires[1 - circle_at];
    use occt::CurveType::{BSpline, Circle, Ellipse, Line};
    // The other profile: a circle, or an ellipse to a profile of one closed edge without connections (batch 188 c004:
    // each along its own parameter from its start).
    let [circle_edge] = &wires[circle_at].sub_shapes(ShapeKind::Edge)[..] else { return Ok(None) };
    let to_ellipse = match circle_edge.curve_type() {
        Ok(Circle) => false,
        Ok(Ellipse) => true,
        _ => return Ok(None),
    };
    let conic = circle_edge.curve_definition().map_err(occt_error)?;
    let (centre, radius) = (conic.frame.origin, conic.radius1);
    let listed = profile.sub_shapes(ShapeKind::Edge);
    if listed.is_empty() || (to_ellipse && (listed.len() > 1 || !connections.is_empty())) {
        return Ok(None);
    }
    if listed.iter().any(|e| !matches!(e.curve_type(), Ok(Line | Circle | Ellipse | BSpline)))
        || listed.iter().all(|e| e.curve_type().ok() == Some(Line))
        || (listed.len() == 1 && listed[0].curve_type().ok() == Some(Circle))
    {
        return Ok(None);
    }
    // Each piece's point and slope at a fraction of its own parameter, as the loop runs.
    let point = |e: &Shape, s: f64| -> R<Vec3> { Ok(e.edge_tangent(s, false).map_err(occt_error)?.origin) };
    let closed = listed.len() == 1;
    let slope = |e: &Shape, s: f64| -> R<Vec3> {
        let h = 1e-6;
        if closed || (s - h >= 0.0 && s + h <= 1.0) {
            let (a, b) = (point(e, (s - h).rem_euclid(1.0))?, point(e, (s + h).rem_euclid(1.0))?);
            return Ok(scale(sub(b, a), 0.5 / h));
        }
        let k = if s - h < 0.0 { 1.0 } else { -1.0 };
        let (p0, p1, p2) = (point(e, s)?, point(e, s + k * h)?, point(e, s + 2.0 * k * h)?);
        Ok(scale(add(scale(p0, -3.0), sub(scale(p1, 4.0), p2)), k * 0.5 / h))
    };
    // The edges in order round the loop, each as it runs (the list isn't in that order).
    let mut edges = vec![listed[0].clone()];
    while edges.len() < listed.len() {
        let end = point(&edges[edges.len() - 1], 1.0)?;
        let Some(next) = listed.iter().find(|e| {
            !edges.iter().any(|x| norm(sub(point(x, 0.5).unwrap_or(centre), point(e, 0.5).unwrap_or(centre))) < 1e-12)
                && point(e, 0.0).is_ok_and(|p| norm(sub(p, end)) < 1e-9)
        }) else {
            return Ok(None);
        };
        edges.push(next.clone());
    }
    // In one plane, parallel to the circle's.
    let normal = conic.frame.normal;
    for e in &edges {
        for s in [0.0, 0.3, 0.7] {
            if dot(sub(point(e, s)?, point(&edges[0], 0.0)?), normal).abs() > 1e-9 {
                return Ok(None);
            }
        }
    }
    // Smooth all the way round.
    for k in 0..edges.len() {
        let (a, b) = (&edges[k], &edges[(k + 1) % edges.len()]);
        let (ta, tb) = (normalize(slope(a, 1.0)?), normalize(slope(b, 0.0)?));
        if norm(sub(point(a, 1.0)?, point(b, 0.0)?)) > 1e-9
            || !matches!((ta, tb), (Some(x), Some(y)) if norm(sub(x, y)) < 1e-6)
        {
            return Ok(None);
        }
    }
    // Anticlockwise along the way from the first profile to the second.
    let samples: Vec<Vec3> = edges
        .iter()
        .flat_map(|e| (0..8).map(move |i| (e.clone(), i as f64 / 8.0)))
        .map(|(e, s)| point(&e, s))
        .collect::<R<_>>()?;
    let centroid = scale(samples.iter().fold(Vec3::new(0.0, 0.0, 0.0), |a, &p| add(a, p)), 1.0 / samples.len() as f64);
    let travel = if circle_at == 1 { sub(centre, centroid) } else { sub(centroid, centre) };
    if dot(travel, normal).abs() < 1e-12 {
        return Ok(None);
    }
    let axis = if dot(travel, normal) > 0.0 { normal } else { scale(normal, -1.0) };
    let turning: f64 = (0..samples.len())
        .map(|i| dot(cross(sub(samples[i], centroid), sub(samples[(i + 1) % samples.len()], centroid)), axis))
        .sum();
    // The first edge's drawn start (its curve's own start), before the loop is turned round.
    let first_start = {
        let e = &edges[0];
        point(e, if e.is_reversed() { 1.0 } else { 0.0 })?
    };
    let reversed = turning < 0.0;
    if reversed {
        edges.reverse();
    }
    // Along the loop as it now runs: piece k at fraction s of its way.
    let at = |k: usize, s: f64| -> R<Vec3> { point(&edges[k], if reversed { 1.0 - s } else { s }) };
    let slope_at = |k: usize, s: f64| -> R<Vec3> {
        let d = slope(&edges[k], if reversed { 1.0 - s } else { s })?;
        Ok(if reversed { scale(d, -1.0) } else { d })
    };
    let lengths: Vec<f64> = edges.iter().map(|e| e.length().map_err(occt_error)).collect::<R<_>>()?;
    let total: f64 = lengths.iter().sum();
    // How far along piece k fraction s is (an ellipse's by its arc length).
    let partial = |k: usize, s: f64| -> R<f64> {
        if matches!(edges[k].curve_type(), Ok(Line | Circle)) {
            return Ok(s * lengths[k]);
        }
        let n = 64;
        let mut sum = 0.0;
        for i in 0..n {
            let (a, b) = (s * i as f64 / n as f64, s * (i + 1) as f64 / n as f64);
            for (x, w) in [(-0.7745966692414834, 5.0 / 9.0), (0.0, 8.0 / 9.0), (0.7745966692414834, 5.0 / 9.0)] {
                sum += w * (b - a) / 2.0 * norm(slope_at(k, (a + b) / 2.0 + x * (b - a) / 2.0)?);
            }
        }
        Ok(sum)
    };
    let before: Vec<f64> = (0..edges.len())
        .scan(0.0, |acc, k| {
            let v = *acc;
            *acc += lengths[k];
            Some(v)
        })
        .collect();
    let fraction = |k: usize, s: f64| -> R<f64> { Ok((before[k] + partial(k, s)?) / total) };
    // The nodes: (fraction round, piece, fraction along it, a vertex?, the circle's angle for a connection).
    let mut nodes: Vec<(f64, usize, f64, bool, Option<Vec3>)> = Vec::new();
    if edges.len() > 1 {
        for k in 0..edges.len() {
            nodes.push((before[k] / total, k, 0.0, true, None));
        }
    }
    let mut first_given = None;
    for ((edge, t), c) in connections {
        // The connection's edge by its middle; its parameter the curve's own, so along the edge as the loop runs.
        let middle = edge.edge_tangent(0.5, false).map_err(occt_error)?.origin;
        let Some(k) = edges.iter().position(|e| point(e, 0.5).is_ok_and(|p| norm(sub(p, middle)) < 1e-9)) else {
            return Ok(None);
        };
        let s = if edges[k].is_reversed() { 1.0 - t } else { *t };
        let s = if reversed { 1.0 - s } else { s };
        let f = fraction(k, s)?;
        first_given.get_or_insert(f);
        match nodes.iter_mut().find(|n| (n.0 - f).abs() < 1e-12 || (n.0 - f).abs() > 1.0 - 1e-12) {
            Some(n) => n.4 = Some(*c),
            None => nodes.push((f, k, s, false, Some(*c))),
        }
    }
    if nodes.is_empty() {
        nodes.push((0.0, 0, 0.0, false, None));
    }
    nodes.sort_by(|a, b| a.0.total_cmp(&b.0));
    // The start: a connection (of two the first given, batch 188 c009; of more the last round from it) or the first
    // edge's drawn start.
    let given = first_given.and_then(|f| {
        nodes.iter().position(|n| (n.0 - f).abs() < 1e-12 || (n.0 - f).abs() > 1.0 - 1e-12)
    });
    if let Some(g) = given {
        nodes.rotate_left(g);
    }
    let marked: Vec<usize> = (0..nodes.len()).filter(|&i| nodes[i].4.is_some()).collect();
    let start = match marked.len() {
        0 => {
            let Some(i) = nodes.iter().position(|n| at(n.1, n.2).is_ok_and(|p| norm(sub(p, first_start)) < 1e-9))
            else {
                return Ok(None);
            };
            i
        }
        1 | 2 => marked[0],
        m => marked[m - 1],
    };
    nodes.rotate_left(start);
    let f0 = nodes[0].0;
    let mut fractions: Vec<f64> = nodes.iter().map(|n| (n.0 - f0).rem_euclid(1.0)).collect();
    fractions[0] = 0.0;
    fractions.push(1.0);
    // The circle: x towards the start (its connection's point, or the start's direction), round anticlockwise.
    let towards = nodes[0].4.unwrap_or(at(nodes[0].1, nodes[0].2)?);
    let d = sub(towards, centre);
    let Some(x) = normalize(sub(d, scale(axis, dot(d, axis)))) else { return Ok(None) };
    let y = cross(axis, x);
    let angle_of = |c: Vec3| {
        let d = sub(c, centre);
        dot(d, y).atan2(dot(d, x)).rem_euclid(tau)
    };
    let n = nodes.len();
    let marks: Vec<usize> = (0..n).filter(|&i| nodes[i].4.is_some()).collect();
    let mut circle_f = vec![0.0; n + 1];
    circle_f[n] = 1.0;
    if marks.len() < 2 {
        for i in 0..n {
            circle_f[i] = i as f64 / n as f64;
        }
    } else {
        for (m, &i) in marks.iter().enumerate() {
            let j = marks.get(m + 1).copied().unwrap_or(n);
            let a = if i == 0 { 0.0 } else { angle_of(nodes[i].4.unwrap_or(centre)) / tau };
            let b = if j < n { angle_of(nodes[j].4.unwrap_or(centre)) / tau } else { 1.0 };
            if b <= a {
                return Err(regen_error("LOFT_CONNECTION_MATCHING"));
            }
            for k in i..j {
                circle_f[k] = a + (b - a) * (k - i) as f64 / (j - i) as f64;
            }
        }
    }
    let Some(round) = Correspondence::new([&fractions, &circle_f], true) else { return Ok(None) };
    // Each interval's piece and the fractions along it at its ends.
    let intervals: Vec<(usize, f64, f64)> = (0..n)
        .map(|i| {
            let (k, s) = (nodes[i].1, nodes[i].2);
            let (nk, ns) = (nodes[(i + 1) % n].1, nodes[(i + 1) % n].2);
            if n == 1 {
                (k, s, s + 1.0)
            } else if nk == k && ns > s {
                (k, s, ns)
            } else if closed && nk == k {
                // Round a closed edge past its start.
                (k, s, ns + 1.0)
            } else {
                (k, s, 1.0)
            }
        })
        .collect();
    let profile_at = |u: f64| -> (Vec3, Vec3) {
        let (f, df) = round.at(0, u);
        let g = (0..n).find(|&g| u <= round.nodes[g + 1]).unwrap_or(n - 1);
        let (k, s0, s1) = intervals[g];
        let (f0, f1) = (fractions[g], fractions[g + 1]);
        let s = s0 + (s1 - s0) * (f - f0) / (f1 - f0);
        let s = if closed { s.rem_euclid(1.0) } else { s.clamp(0.0, 1.0) };
        let p = at(k, s).unwrap_or(centre);
        let d = slope_at(k, s).unwrap_or(centre);
        (p, scale(d, (s1 - s0) / (f1 - f0) * df))
    };
    // An ellipse runs from its start, the way round the loft runs; its start must be where the profile's is headed.
    let ellipse_backwards = to_ellipse && {
        let (p, d) = (point(circle_edge, 0.0)?, slope(circle_edge, 0.0)?);
        dot(cross(sub(p, centre), d), axis) < 0.0
    };
    if to_ellipse {
        let p = point(circle_edge, 0.0)?;
        let Some(dir) = normalize(sub(sub(p, centre), scale(axis, dot(sub(p, centre), axis)))) else { return Ok(None) };
        if norm(sub(dir, x)) > 1e-9 {
            return Ok(None);
        }
    }
    let circle_point = |u: f64| -> (Vec3, Vec3) {
        let (f, df) = round.at(1, u);
        if to_ellipse {
            let s = if ellipse_backwards { (1.0 - f).rem_euclid(1.0) } else { f.rem_euclid(1.0) };
            let p = point(circle_edge, s).unwrap_or(centre);
            let d = slope(circle_edge, s).unwrap_or(centre);
            return (p, scale(d, if ellipse_backwards { -df } else { df }));
        }
        let theta = tau * f;
        let p = add(centre, add(scale(x, radius * theta.cos()), scale(y, radius * theta.sin())));
        let d = add(scale(x, -radius * theta.sin()), scale(y, radius * theta.cos()));
        (p, scale(d, tau * df))
    };
    let tolerance = if connections.is_empty() { 1e-8 } else { connected_span_tolerance(connections.len()) };
    let mut breaks = vec![0.0];
    for g in 0..n {
        let off = |p: f64, q: f64| midpoint_off(&profile_at, p, q).max(midpoint_off(&circle_point, p, q));
        halved_spans(round.nodes[g], round.nodes[g + 1], &off, tolerance, &mut breaks, 0);
    }
    let rows: Vec<Vec<(f64, Vec3, Vec3)>> = [&profile_at as &dyn Fn(f64) -> (Vec3, Vec3), &circle_point]
        .iter()
        .map(|f| breaks.iter().map(|&u| (u, f(u).0, f(u).1)).collect())
        .collect();
    let (profile_row, knots) = hermite_poles(&rows[0]);
    let (circle_row, _) = hermite_poles(&rows[1]);
    let (row0, row1) = if circle_at == 1 { (&profile_row, &circle_row) } else { (&circle_row, &profile_row) };
    let poles: Vec<Vec3> = row0.iter().zip(row1).flat_map(|(&a, &b)| [a, b]).collect();
    let side = occt::u_periodic_bspline_surface_face(&poles, (row0.len(), 2), &knots, &[0.0, 0.0, 1.0, 1.0], (3, 1))
        .map_err(|_| regen_error("LOFT_FAILED"))?;
    // The side's edges: split at the profile's vertices, and with connections at every node, both ends (but not for
    // one connection round a loop without vertices); the seam midway along the first span.
    let has_vertices = edges.len() > 1;
    let split_all = !connections.is_empty() && (has_vertices || connections.len() > 1);
    let profile_splits: Vec<f64> = (1..n)
        .filter(|&i| split_all || nodes[i].3)
        .map(|i| round.nodes[i])
        .chain([1.0].into_iter().filter(|_| split_all || nodes[0].3))
        .collect();
    let circle_splits: Vec<f64> = if split_all { round.nodes[1..].to_vec() } else { Vec::new() };
    let seam = breaks[1] / 2.0;
    let ends =
        |splits: &[f64]| -> Vec<f64> { [seam].into_iter().chain(splits.iter().copied()).chain([1.0 + seam]).collect() };
    let (ends0, ends1) = if circle_at == 1 {
        (ends(&profile_splits), ends(&circle_splits))
    } else {
        (ends(&circle_splits), ends(&profile_splits))
    };
    let mut loop_uv: Vec<Vec<(f64, f64)>> = ends0.windows(2).map(|w| vec![(w[0], 0.0), (w[1], 0.0)]).collect();
    loop_uv.push(vec![(1.0 + seam, 0.0), (1.0 + seam, 1.0)]);
    loop_uv.extend(ends1.windows(2).rev().map(|w| vec![(w[1], 1.0), (w[0], 1.0)]));
    loop_uv.push(vec![(seam, 1.0), (seam, 0.0)]);
    let side = occt::face_from_uv(&side, &loop_uv).map_err(|_| regen_error("LOFT_FAILED"))?;
    // The ends bounded by the side's own edges.
    let mut caps = Vec::new();
    for w in wires {
        let probe = w.sub_shapes(ShapeKind::Edge)[0].edge_tangent(0.0, false).map_err(occt_error)?.origin;
        let side_edges: Vec<Shape> = side
            .sub_shapes(ShapeKind::Edge)
            .into_iter()
            .filter(|e| e.edge_tangent(0.5, false).is_ok_and(|a| dot(sub(a.origin, probe), normal).abs() < 1e-9))
            .collect();
        caps.push(occt::fill_edges(&side_edges).map_err(|_| regen_error("LOFT_FAILED"))?);
    }
    let sewn =
        occt::sew_faces(&[side, caps[0].clone(), caps[1].clone()], 1e-6).map_err(|_| regen_error("LOFT_FAILED"))?;
    let shape = occt::solid_from(&sewn).map_err(|_| regen_error("LOFT_FAILED"))?;
    let shape = if shape.volume().map_err(occt_error)? < 0.0 { shape.reversed() } else { shape };
    let find = |c: &Shape| -> Vec<Shape> {
        let Ok(p) = c.face_inner_point() else { return Vec::new() };
        shape
            .sub_shapes(ShapeKind::Face)
            .into_iter()
            .filter(|f| f.surface_type().ok() == Some(occt::SurfaceType::Plane))
            .filter(|f| occt::vertex(p).ok().and_then(|v| v.distance(f).ok()).is_some_and(|d| d.distance < 1e-7))
            .collect()
    };
    Ok(Some((shape.clone(), find(&caps[0]), find(&caps[1]))))
}

/// Two polygons with different numbers of corners, matched as Onshape lofts them (batches 180 c007, 181 c001–c006,
/// 182 c001, c002, 187 c001–c003): from the first one's first corner to the corner of the other nearest it (of two as near, the
/// later round), or from each connection's corners to the next's, both run anticlockwise along the way from the
/// first to the second and measured by the fraction of each stretch from there; in each stretch each corner of the
/// polygon with fewer goes to the other's corner nearest it so measured (ties as below), and each of the other's
/// corners left over to the middle of the edge it falls in (several in one edge, evenly along it). The two polygons
/// rebuilt from those corners, points added; `None` for other profiles.
fn unequal_polygons(wires: &[Shape], forced: &[(Vec3, Vec3)]) -> R<Option<Vec<Shape>>> {
    use values::{add, cross, dot, norm, normalize, scale, sub};
    let [first, second] = wires else { return Ok(None) };
    let (Some(mut p), Some(mut q)) = (polygon_corners(first), polygon_corners(second)) else { return Ok(None) };
    if p.len() == q.len() && forced.len() < 2 {
        return Ok(None);
    }
    let centre = |c: &[Vec3]| scale(c.iter().fold(Vec3::new(0.0, 0.0, 0.0), |a, &x| add(a, x)), 1.0 / c.len() as f64);
    let (cp, cq) = (centre(&p), centre(&q));
    let Some(axis) = normalize(sub(cq, cp)) else { return Ok(None) };
    // Onshape's first corner is where the region's first edge, as the engine's loop runs, ends.
    let start = p[1];
    let anticlockwise = |c: &mut Vec<Vec3>, o: Vec3| {
        if dot(cross(sub(c[0], o), sub(c[1], o)), axis) < 0.0 {
            c.reverse();
        }
    };
    anticlockwise(&mut p, cp);
    anticlockwise(&mut q, cq);
    // Of corners as near, the later round (batch 181 c001, c003: the square's at 45° to the triangle's at 90°, the
    // hexagon's at 120°).
    let nearest = |c: &[Vec3], x: Vec3| -> usize {
        let d: Vec<f64> = c.iter().map(|&v| norm(sub(v, x))).collect();
        let least = d.iter().copied().fold(f64::INFINITY, f64::min);
        (0..c.len()).rev().find(|&i| d[i] - least < 1e-9).unwrap_or(0)
    };
    let mut anchors: Vec<(usize, usize)> = if forced.is_empty() {
        let ip = nearest(&p, start);
        vec![(ip, nearest(&q, p[ip]))]
    } else {
        forced.iter().map(|&(a, b)| (nearest(&p, a), nearest(&q, b))).collect()
    };
    // Round from the first.
    let (p0, q0) = anchors[0];
    let (np, nq) = (p.len(), q.len());
    let p: Vec<Vec3> = (0..np).map(|i| p[(i + p0) % np]).collect();
    let q: Vec<Vec3> = (0..nq).map(|i| q[(i + q0) % nq]).collect();
    for a in anchors.iter_mut() {
        *a = ((a.0 + np - p0) % np, (a.1 + nq - q0) % nq);
    }
    anchors.sort_by_key(|a| a.0);
    // The connections must run round both the same way.
    if anchors.windows(2).any(|w| w[1].1 <= w[0].1) {
        return Ok(None);
    }
    // One stretch: corners a..=b of one and c..=d of the other (indices round), matched by fraction.
    let stretch = |c: &[Vec3], from: usize, to: usize| -> Vec<Vec3> { (from..=to).map(|k| c[k % c.len()]).collect() };
    // Fractions of the whole perimeter from the first anchor, summed edge by edge.
    let fractions = |c: &[Vec3]| -> Vec<f64> {
        let n = c.len();
        let lengths: Vec<f64> = (0..n).map(|i| norm(sub(c[(i + 1) % n], c[i]))).collect();
        let total: f64 = lengths.iter().sum();
        let mut at = vec![0.0];
        for l in &lengths[..n - 1] {
            at.push(at.last().copied().unwrap_or(0.0) + l / total);
        }
        at.push(1.0);
        at
    };
    let (fp, fq) = (fractions(&p), fractions(&q));
    let mut grown_p = Vec::new();
    let mut grown_q = Vec::new();
    for k in 0..anchors.len() {
        let (a, c) = anchors[k];
        // The last stretch runs on round to the first anchor (its indices a turn on).
        let (b, d) = if k + 1 < anchors.len() { anchors[k + 1] } else { (np, nq) };
        let (sp, sq) = (stretch(&p, a, b), stretch(&q, c, d));
        let small_is_p = sp.len() <= sq.len();
        let (small, large) = if small_is_p { (&sp, &sq) } else { (&sq, &sp) };
        let (fs, fl): (Vec<f64>, Vec<f64>) = if small_is_p {
            ((a..=b).map(|i| fp[i]).collect(), (c..=d).map(|i| fq[i]).collect())
        } else {
            ((c..=d).map(|i| fq[i]).collect(), (a..=b).map(|i| fp[i]).collect())
        };
        // Each corner of the smaller to the larger's nearest by fraction, in order. Of two as near, the earlier (181
        // c004, a square to a pentagon), but in the stretch from the last connection round to the first the later
        // when there are two connections or more (182 c001, 187 c002: the earlier in the first stretch, the later in
        // the second), and with one when the polygon with fewer corners is the second (187 c001; c003, the fewer
        // first, the earlier). Seven ties fit this; why Onshape breaks them so isn't known.
        let later = !forced.is_empty() && k + 1 == anchors.len() && (anchors.len() > 1 || !small_is_p);
        let mut partner = vec![0usize; small.len()];
        let last = small.len() - 1;
        partner[last] = large.len() - 1;
        for i in 1..last {
            let lo = partner[i - 1] + 1;
            let hi = large.len() - 1 - (last - i);
            if lo > hi {
                return Ok(None);
            }
            let off = |x: usize| (fl[x] - fs[i]).abs();
            let least = (lo..=hi).map(off).fold(f64::INFINITY, f64::min);
            let near: Vec<usize> = (lo..=hi).filter(|&x| off(x) - least < 1e-9).collect();
            partner[i] = if later { near[near.len() - 1] } else { near[0] };
        }
        // The smaller with the larger's corners left over in the middle of the edge each falls in (the stretch's last
        // corner is the next stretch's first, so left out).
        let mut grown = Vec::new();
        for i in 0..last {
            let (x, y) = (small[i], small[i + 1]);
            let extra = partner[i + 1] - partner[i] - 1;
            grown.push(x);
            for e in 1..=extra {
                grown.push(add(x, scale(sub(y, x), e as f64 / (extra + 1) as f64)));
            }
        }
        let rest: Vec<Vec3> = large[..large.len() - 1].to_vec();
        if small_is_p {
            grown_p.extend(grown);
            grown_q.extend(rest);
        } else {
            grown_q.extend(grown);
            grown_p.extend(rest);
        }
    }
    if grown_p.len() != grown_q.len() {
        return Ok(None);
    }
    let rebuild = |c: &[Vec3]| -> R<Shape> {
        let edges: Vec<Shape> =
            (0..c.len()).map(|i| occt::line_edge(c[i], c[(i + 1) % c.len()])).collect::<Result<_, _>>().map_err(occt_error)?;
        occt::ordered_wire(&edges).map_err(occt_error)
    };
    Ok(Some(vec![rebuild(&grown_p)?, rebuild(&grown_q)?]))
}

/// A closed wire of straight edges with points along its edges made corners (none: the wire itself), or `None` for
/// other wires.
fn with_corners(wire: &Shape, points: &[Vec3]) -> R<Option<Shape>> {
    use values::{cross, dot, norm, sub};
    if points.is_empty() {
        return Ok(polygon_corners(wire).map(|_| wire.clone()));
    }
    let Some(corners) = polygon_corners(wire) else { return Ok(None) };
    let n = corners.len();
    let mut grown = Vec::new();
    for k in 0..n {
        let (a, b) = (corners[k], corners[(k + 1) % n]);
        grown.push(a);
        let e = sub(b, a);
        let mut along: Vec<(f64, Vec3)> = points
            .iter()
            .filter(|&&p| {
                let d = sub(p, a);
                norm(cross(d, e)) < 1e-9 * norm(e) && dot(d, e) > 1e-9 * dot(e, e) && dot(d, e) < (1.0 - 1e-9) * dot(e, e)
            })
            .map(|&p| (dot(sub(p, a), e), p))
            .collect();
        along.sort_by(|x, y| x.0.total_cmp(&y.0));
        grown.extend(along.into_iter().map(|(_, p)| p));
    }
    let edges: Vec<Shape> =
        (0..grown.len()).map(|i| occt::line_edge(grown[i], grown[(i + 1) % grown.len()])).collect::<Result<_, _>>().map_err(occt_error)?;
    Ok(Some(occt::ordered_wire(&edges).map_err(occt_error)?))
}

/// The corners of a closed wire of straight edges, in order round it.
fn polygon_corners(wire: &Shape) -> Option<Vec<Vec3>> {
    let edges = wire.sub_shapes(ShapeKind::Edge);
    if edges.len() < 3 || edges.iter().any(|e| e.curve_type().ok() != Some(occt::CurveType::Line)) {
        return None;
    }
    let ends: Vec<(Vec3, Vec3)> = edges
        .iter()
        .map(|e| Some((e.edge_tangent(0.0, false).ok()?.origin, e.edge_tangent(1.0, false).ok()?.origin)))
        .collect::<Option<_>>()?;
    let close = |a: Vec3, b: Vec3| values::norm(values::sub(a, b)) < 1e-9;
    let mut corners = vec![ends[0].0, ends[0].1];
    let mut used = vec![false; ends.len()];
    used[0] = true;
    while corners.len() < ends.len() {
        let last = *corners.last()?;
        let k = (0..ends.len()).find(|&k| !used[k] && (close(ends[k].0, last) || close(ends[k].1, last)))?;
        used[k] = true;
        corners.push(if close(ends[k].0, last) { ends[k].1 } else { ends[k].0 });
    }
    Some(corners)
}

/// Two polygons with as many corners, matched corner to corner as Onshape lofts them (batch 074 c001): running the
/// same way round the loft's axis, each corner to the other's nearest (least total distance), and where two
/// matchings are as near (a square to one turned 45°), each corner to the one anticlockwise of it seen along the
/// axis. The second polygon is rebuilt from its matching corner. `None` for other profiles.
fn matched_polygons(wires: &[Shape]) -> R<Option<Vec<Shape>>> {
    matched_polygons_with(wires, None)
}

/// [`matched_polygons`], or with the corners nearest two points (a connection's, batch 075 c001–c003) matched to
/// each other and the rest in turn from them.
fn matched_polygons_with(wires: &[Shape], forced: Option<(Vec3, Vec3)>) -> R<Option<Vec<Shape>>> {
    use values::{add, cross, dot, norm, normalize, scale, sub};
    let [first, second] = wires else { return Ok(None) };
    let (Some(p), Some(mut q)) = (polygon_corners(first), polygon_corners(second)) else { return Ok(None) };
    if p.len() != q.len() {
        return Ok(None);
    }
    let n = p.len();
    let centre = |c: &[Vec3]| scale(c.iter().fold(Vec3::new(0.0, 0.0, 0.0), |s, x| add(s, *x)), 1.0 / c.len() as f64);
    let (cp, cq) = (centre(&p), centre(&q));
    let Some(axis) = normalize(sub(cq, cp)) else { return Ok(None) };
    // Which way round the axis a polygon runs.
    let sense = |c: &[Vec3], o: Vec3| dot(cross(sub(c[0], o), sub(c[1], o)), axis);
    if sense(&p, cp) * sense(&q, cq) < 0.0 {
        q.reverse();
    }
    let cost = |k: usize| (0..n).map(|i| norm(sub(p[i], q[(i + k) % n])).powi(2)).sum::<f64>();
    let best = (0..n).map(cost).fold(f64::INFINITY, f64::min);
    let ties: Vec<usize> = (0..n).filter(|&k| cost(k) <= best * (1.0 + 1e-9) + 1e-18).collect();
    // Of equal matchings, the one turning anticlockwise about the axis from p's first corner to its match.
    let turn = |k: usize| {
        let (a, b) = (sub(p[0], cp), sub(q[k % n], cq));
        let (a, b) = (sub(a, scale(axis, dot(a, axis))), sub(b, scale(axis, dot(b, axis))));
        dot(cross(a, b), axis).atan2(dot(a, b))
    };
    let nearest = |c: &[Vec3], x: Vec3| (0..c.len()).min_by(|&a, &b| norm(sub(c[a], x)).total_cmp(&norm(sub(c[b], x))));
    let k = match forced {
        Some((a, b)) => {
            let (Some(i), Some(j)) = (nearest(&p, a), nearest(&q, b)) else { return Ok(None) };
            (j + n - i) % n
        }
        None => {
            let Some(&k) = ties.iter().max_by(|&&a, &&b| turn(a).total_cmp(&turn(b))) else { return Ok(None) };
            k
        }
    };
    let rotated: Vec<Vec3> = (0..n).map(|i| q[(i + k) % n]).collect();
    let rebuild = |c: &[Vec3]| -> R<Shape> {
        let edges: Vec<Shape> =
            (0..c.len()).map(|i| occt::line_edge(c[i], c[(i + 1) % c.len()])).collect::<Result<_, _>>().map_err(occt_error)?;
        occt::ordered_wire(&edges).map_err(occt_error)
    };
    Ok(Some(vec![rebuild(&p)?, rebuild(&rotated)?]))
}

pub fn loft(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    // Sections added along a spine that isn't straight: Onshape fails (circles along a quarter circle, either way
    // round, along a longer arc: `LOFT_FAILED`; with two sections `LOFT_INVALID`, batches 074 c006, 075 c007, 089
    // c006-c008). Along a straight spine the loft is the same as without it (089 c009).
    if values::opt_bool(definition, "addSections")?.unwrap_or(false) && !field(definition, "spine").is_undefined() {
        let spine = query::evaluate(model, field(definition, "spine"))?;
        let straight = spine
            .iter()
            .filter(|r| r.kind == EntityType::Edge)
            .all(|r| model.shape(*r).curve_type().ok() == Some(occt::CurveType::Line));
        if !straight {
            let sections = field(definition, "sectionCount").as_number().unwrap_or(0.0);
            return Err(regen_error(if sections <= 2.0 { "LOFT_INVALID" } else { "LOFT_FAILED" }));
        }
    }
    let has = |key: &str| field(definition, key).as_array().is_some_and(|a| !a.is_empty());
    let (has_guides, has_connections) = (has("guideSubqueries"), has("connections"));
    // Conditions at the ends: a derivative along a vector (normal to the profile's plane, or a direction given) or
    // in the plane square to it (tangent to the profile's plane).
    // Matching the tangency of the faces next to the profile: along where they meet (batch 058).
    let mut ends: [Option<occt::LoftEnd>; 2] = [None, None];
    for info in field(definition, "derivativeInfo").as_array().map(|a| a.to_vec()).unwrap_or_default() {
        let index =
            field(&info, "profileIndex").as_number().ok_or_else(|| error("@opLoft: profileIndex must be a number"))?;
        let magnitude = field(&info, "magnitude").as_number().unwrap_or(1.0);
        if !field(&info, "adjacentFaces").is_undefined() {
            let curvature = values::opt_bool(&info, "matchCurvature")?.unwrap_or(false);
            let faces: Vec<Shape> = query::evaluate(model, field(&info, "adjacentFaces"))?
                .into_iter()
                .filter(|r| r.kind == EntityType::Face)
                .map(|r| model.shape(r).clone())
                .collect();
            if faces.is_empty() {
                return Err(regen_error(if index == 0.0 {
                    "LOFT_NO_FACE_FOR_START_CLAMP"
                } else {
                    "LOFT_NO_FACE_FOR_END_CLAMP"
                }));
            }
            ends[if index == 0.0 { 0 } else { 1 }] =
                Some(occt::LoftEnd { vector: Vec3::new(0.0, 0.0, 1.0), magnitude, tangent: false, faces, curvature });
            continue;
        }
        let tangent = values::opt_bool(&info, "tangentToPlane")?.unwrap_or(false);
        let vector = values::vec3(field(&info, "vector"), "@opLoft derivative vector")?;
        ends[if index == 0.0 { 0 } else { 1 }] =
            Some(occt::LoftEnd { vector, magnitude, tangent, faces: Vec::new(), curvature: false });
    }
    let solid = field(definition, "bodyType").as_str() != Some("SURFACE");
    let subqueries = field(definition, "profileSubqueries")
        .as_array()
        .ok_or_else(|| error("@opLoft: profileSubqueries must be an array"))?;
    let mut wires = Vec::new();
    // The first profile's one open edge, as Onshape's runs: a point on it and its direction there.
    let mut open_start: Option<(Vec3, Vec3)> = None;
    for q in subqueries.iter() {
        let found = query::evaluate(model, q)?;
        let faces: Vec<&_> = found.iter().filter(|r| r.kind == EntityType::Face).collect();
        if let Some(face) = faces.first() {
            // Several faces, a region split by lines across it: merged into one, its boundary's collinear edges one
            // edge each, as Onshape's profile is (batch 169: a rectangle split in two lofts as the rectangle).
            let face = if faces.len() > 1 {
                let shapes: Vec<Shape> = faces.iter().map(|r| model.shape(**r).clone()).collect();
                let merged = occt::merge_faces(&shapes).map_err(occt_error)?.shape();
                let [one] = &merged.sub_shapes(ShapeKind::Face)[..] else {
                    return Err(error("@opLoft: profiles of several faces that don't make one aren't supported yet"));
                };
                let count = |s: &Shape| s.sub_shapes(ShapeKind::Edge).len();
                match occt::merge_edges(one) {
                    Ok(op) if count(&op.shape()) < count(one) => op.shape().sub_shapes(ShapeKind::Face).into_iter().next().unwrap_or(one.clone()),
                    _ => one.clone(),
                }
            } else {
                model.shape(**face).clone()
            };
            wires.push(occt::outer_wire(&face).map_err(occt_error)?);
            continue;
        }
        let edges: Vec<Shape> =
            found.iter().filter(|r| r.kind == EntityType::Edge).map(|r| model.shape(*r).clone()).collect();
        if edges.is_empty() {
            // A point profile (a vertex), at either end.
            let vertices: Vec<&_> = found.iter().filter(|r| r.kind == EntityType::Vertex).collect();
            let [vertex] = vertices[..] else { return Err(regen_error("LOFT_PROFILE_FAILED")) };
            wires.push(model.shape(*vertex).clone());
            continue;
        }
        if wires.is_empty()
            && let [r] = found.iter().filter(|r| r.kind == EntityType::Edge).collect::<Vec<_>>()[..]
        {
            let edge = super::onshape_edge(model, *r);
            let ends = [0.0, 1.0].map(|t| edge.edge_tangent(t, false).map(|a| a.origin));
            if let ([Ok(a), Ok(b)], Ok(mid)) = (ends, edge.edge_tangent(0.5, false))
                && values::norm(values::sub(a, b)) > 1e-9
            {
                open_start = Some((mid.origin, mid.direction));
            }
        }
        let mut connected = occt::connect_edges(&edges).map_err(occt_error)?;
        if connected.len() != 1 {
            return Err(regen_error("LOFT_PROFILE_FAILED"));
        }
        wires.push(connected.remove(0));
    }
    if wires.len() < 2 {
        return Err(regen_error("LOFT_SELECT_PROFILES"));
    }
    // Normal to a direction, the loft is tangent to the plane square to it along the profile: the profile must lie
    // in such a plane (batch 056 c006: a circle in xy, normal to a direction at 45°, is LOFT_INVALID).
    for (end, wire) in [(&ends[0], &wires[0]), (&ends[1], &wires[wires.len() - 1])] {
        let Some(e) = end.as_ref().filter(|e| e.tangent && e.faces.is_empty()) else { continue };
        let Some(n) = values::normalize(e.vector) else { continue };
        let points: Vec<Vec3> = wire
            .sub_shapes(ShapeKind::Edge)
            .iter()
            .flat_map(|edge| [0.0, 0.25, 0.5, 0.75, 1.0].map(|t| edge.edge_tangent(t, false).ok().map(|a| a.origin)))
            .flatten()
            .collect();
        if let Some(&first) = points.first()
            && points.iter().any(|&p| values::dot(values::sub(p, first), n).abs() > 1e-7)
        {
            return Err(regen_error("LOFT_INVALID"));
        }
    }
    let touches = |a: &Shape, b: &Shape| a.distance(b).is_ok_and(|d| d.distance < 1e-7);
    // Each connection takes one vertex or edge on each profile (batch 074 c002–c004: a corner's every vertex,
    // the sketch lines' ends too, is `LOFT_CONNECTION_MATCHING`).
    if has_connections {
        for connection in field(definition, "connections").as_array().map(|a| a.to_vec()).unwrap_or_default() {
            let entities: Vec<Shape> =
                query::evaluate(model, field(&connection, "connectionEntities"))?.into_iter().map(|r| model.shape(r).clone()).collect();
            let one_each = entities.len() == wires.len()
                && wires.iter().all(|w| entities.iter().filter(|e| touches(e, w)).count() == 1);
            if !one_each {
                return Err(regen_error("LOFT_CONNECTION_MATCHING"));
            }
        }
        // Two polygons connected corner to corner: that matching (batch 075 c001–c003); any connection along an
        // edge is matched below.
        let first = field(definition, "connections").as_array().and_then(|a| a.first().cloned()).unwrap_or(Value::UNDEFINED);
        let entities: Vec<Shape> =
            query::evaluate(model, field(&first, "connectionEntities"))?.into_iter().map(|r| model.shape(r).clone()).collect();
        let point = |w: &Shape| -> Option<Vec3> { entities.iter().find(|e| touches(e, w))?.vertex_point().ok() };
        let mut corners_only = true;
        for connection in field(definition, "connections").as_array().map(|a| a.to_vec()).unwrap_or_default() {
            for r in query::evaluate(model, field(&connection, "connectionEntities"))? {
                corners_only &= r.kind == EntityType::Vertex;
            }
        }
        let forced = match (wires.len(), point(&wires[0]), wires.get(1).and_then(point)) {
            (2, Some(a), Some(b)) if corners_only => Some((a, b)),
            _ => None,
        };
        // Two circles connected at points along them (batch 082 c006): each started there, lofted start to start
        // (a quarter along one and half along the other, a quarter turn's twist).
        let circles: Vec<(occt::Conic, Shape)> = wires
            .iter()
            .filter_map(|w| match &w.sub_shapes(ShapeKind::Edge)[..] {
                [e] if e.curve_type().ok() == Some(occt::CurveType::Circle) => {
                    Some((e.curve_definition().ok()?, e.clone()))
                }
                _ => None,
            })
            .collect();
        // Two circles connected once (batch 082 c006): each started at the connection's point, running as its own
        // circle does, lofted start to start (a quarter along one and half along the other, a quarter turn's twist).
        if circles.len() == 2 && wires.len() == 2 {
            use crate::values::{cross, dot, normalize, sub};
            if ends.iter().any(Option::is_some) {
                return Err(error("@opLoft: connections with end conditions aren't supported yet"));
            }
            // Each connection's point round each circle.
            let mut points: Vec<Vec<Vec3>> = vec![Vec::new(), Vec::new()];
            for connection in field(definition, "connections").as_array().map(|a| a.to_vec()).unwrap_or_default() {
                let params: Vec<f64> = field(&connection, "connectionEdgeParameters")
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_number).collect())
                    .unwrap_or_default();
                let edges: Vec<Shape> = query::evaluate(model, field(&connection, "connectionEdgeQueries"))?
                    .into_iter()
                    .filter(|r| r.kind == EntityType::Edge)
                    .map(|r| model.shape(r).clone())
                    .collect();
                for (c, (_, edge)) in circles.iter().enumerate() {
                    let k = edges.iter().position(|e| touches(e, edge)).ok_or_else(|| regen_error("LOFT_CONNECTION_MATCHING"))?;
                    let t = *params.get(k).ok_or_else(|| regen_error("LOFT_CONNECTION_MATCHING"))?;
                    points[c].push(edges[k].edge_tangent(t, true).map_err(occt_error)?.origin);
                }
            }
            let mut frames = Vec::new();
            for (c, (conic, edge)) in circles.iter().enumerate() {
                // Running as the profile's own circle does.
                let start = edge.edge_tangent(0.0, false).map_err(occt_error)?;
                let normal = normalize(cross(sub(start.origin, conic.frame.origin), start.direction))
                    .ok_or_else(|| regen_error("LOFT_FAILED"))?;
                let x = normalize(sub(points[c][0], conic.frame.origin)).ok_or_else(|| regen_error("LOFT_FAILED"))?;
                frames.push((occt::Frame { origin: conic.frame.origin, normal, x }, conic.radius1));
            }
            // Connected more than once: one periodic side (batch 182 c004).
            if points[0].len() > 1 {
                let tau = std::f64::consts::TAU;
                let angles: Vec<Vec<f64>> = (0..2)
                    .map(|c| {
                        let (frame, _) = &frames[c];
                        let y = cross(frame.normal, frame.x);
                        let mut a: Vec<f64> = points[c]
                            .iter()
                            .map(|&p| {
                                let d = sub(p, frame.origin);
                                dot(d, y).atan2(dot(d, frame.x)).rem_euclid(tau)
                            })
                            .collect();
                        a[0] = 0.0;
                        a
                    })
                    .collect();
                // In order round the first circle, and so round the second.
                let mut order: Vec<usize> = (0..angles[0].len()).collect();
                order.sort_by(|&a, &b| angles[0][a].total_cmp(&angles[0][b]));
                let sorted: Vec<Vec<f64>> = angles.iter().map(|a| order.iter().map(|&k| a[k]).collect()).collect();
                if sorted.iter().any(|a| a.windows(2).any(|w| w[1] <= w[0] + 1e-9)) {
                    return Err(regen_error("LOFT_CONNECTION_MATCHING"));
                }
                let built = circles_connected([frames[0].clone(), frames[1].clone()], [&sorted[0], &sorted[1]], solid)?;
                let Some((shape, first, last)) = built else {
                    return Err(error("@opLoft: circles connected more than once into a surface aren't supported yet"));
                };
                add_lofted_shape(model, &id, &shape, &first, &last, solid, &wires)?;
                return Ok(Value::UNDEFINED);
            }
            let mut started = Vec::new();
            for (frame, radius) in frames {
                let circle = occt::circle_edge(frame, radius).map_err(occt_error)?;
                started.push(occt::ordered_wire(&[circle]).map_err(occt_error)?);
            }
            let op = occt::loft_as_given(&started, solid).map_err(|_| regen_error("LOFT_FAILED"))?;
            add_lofted(model, &id, &op, solid, &wires)?;
            return Ok(Value::UNDEFINED);
        }
        // A polygon and a circle joined by connections (batches 180, 181).
        if wires.len() == 2 && circles.len() == 1 {
            if ends.iter().any(Option::is_some) {
                return Err(error("@opLoft: connections with end conditions aren't supported yet"));
            }
            let polygon = if touches(&wires[0], &circles[0].1) { 1 } else { 0 };
            // A smooth profile (a slot, an ellipse) to the circle: one periodic side (batch 187 c010, c011). Each
            // connection's edge and parameter on the profile (a vertex as the start or end of an edge there), and
            // its point round the circle.
            if polygon_corners(&wires[polygon]).is_none() {
                let mut joined = Vec::new();
                for connection in field(definition, "connections").as_array().map(|a| a.to_vec()).unwrap_or_default() {
                    let entities: Vec<Shape> = query::evaluate(model, field(&connection, "connectionEntities"))?
                        .into_iter()
                        .map(|r| model.shape(r).clone())
                        .collect();
                    let params: Vec<f64> = field(&connection, "connectionEdgeParameters")
                        .as_array()
                        .map(|a| a.iter().filter_map(Value::as_number).collect())
                        .unwrap_or_default();
                    let edges: Vec<Shape> = query::evaluate(model, field(&connection, "connectionEdgeQueries"))?
                        .into_iter()
                        .filter(|r| r.kind == EntityType::Edge)
                        .map(|r| model.shape(r).clone())
                        .collect();
                    let on_circle = edges.iter().zip(&params).find(|(e, _)| touches(e, &circles[0].1));
                    let on_profile = match entities
                        .iter()
                        .find(|e| e.kind() == Some(ShapeKind::Vertex) && touches(e, &wires[polygon]))
                    {
                        Some(v) => {
                            let p = v.vertex_point().map_err(occt_error)?;
                            wires[polygon].sub_shapes(ShapeKind::Edge).into_iter().find_map(|e| {
                                let native = |t: f64| {
                                    e.edge_tangent(if e.is_reversed() { 1.0 - t } else { t }, false)
                                        .ok()
                                        .map(|a| a.origin)
                                };
                                [0.0, 1.0]
                                    .into_iter()
                                    .find(|&t| native(t).is_some_and(|q| values::norm(values::sub(q, p)) < 1e-9))
                                    .map(|t| (e.clone(), t))
                            })
                        }
                        None => edges
                            .iter()
                            .zip(&params)
                            .find(|(e, _)| touches(e, &wires[polygon]))
                            .map(|(e, &t)| (e.clone(), t)),
                    };
                    match (on_profile, on_circle) {
                        (Some(at), Some((e, &t))) => {
                            joined.push((at, e.edge_tangent(t, false).map_err(occt_error)?.origin))
                        }
                        _ => return Err(regen_error("LOFT_CONNECTION_MATCHING")),
                    }
                }
                let circle_at = 1 - polygon;
                if let Some((shape, first, last)) = smooth_loft_to_circle(&wires, circle_at, &joined, solid)? {
                    add_lofted_shape(model, &id, &shape, &first, &last, solid, &wires)?;
                    return Ok(Value::UNDEFINED);
                }
            }
            // Each connection's point on the polygon (a corner, or along an edge) and round the circle.
            let mut pairs = Vec::new();
            for connection in field(definition, "connections").as_array().map(|a| a.to_vec()).unwrap_or_default() {
                let entities: Vec<Shape> = query::evaluate(model, field(&connection, "connectionEntities"))?
                    .into_iter()
                    .map(|r| model.shape(r).clone())
                    .collect();
                let params: Vec<f64> = field(&connection, "connectionEdgeParameters")
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_number).collect())
                    .unwrap_or_default();
                let edges: Vec<Shape> = query::evaluate(model, field(&connection, "connectionEdgeQueries"))?
                    .into_iter()
                    .filter(|r| r.kind == EntityType::Edge)
                    .map(|r| model.shape(r).clone())
                    .collect();
                let along = |w: &Shape| -> R<Option<Vec3>> {
                    edges
                        .iter()
                        .zip(&params)
                        .find(|(e, _)| touches(e, w))
                        .map(|(e, &t)| e.edge_tangent(t, true).map(|a| a.origin))
                        .transpose()
                        .map_err(occt_error)
                };
                let corner = entities
                    .iter()
                    .find(|e| e.kind() == Some(ShapeKind::Vertex) && touches(e, &wires[polygon]))
                    .and_then(|v| v.vertex_point().ok());
                let at_polygon = match corner {
                    Some(p) => Some(p),
                    None => along(&wires[polygon])?,
                };
                match (at_polygon, along(&circles[0].1)?) {
                    (Some(p), Some(c)) => pairs.push((p, c)),
                    _ => return Err(regen_error("LOFT_CONNECTION_MATCHING")),
                }
            }
            if let Some((shape, first, last)) = polygon_circle_connected(&wires, polygon, &circles[0].0, &pairs, solid)? {
                add_lofted_shape(model, &id, &shape, &first, &last, solid, &wires)?;
                return Ok(Value::UNDEFINED);
            }
        }
        let matched = match forced.map(|f| matched_polygons_with(&wires, Some(f))).transpose()?.flatten() {
            Some(m) => Some(m),
            None if wires.len() == 2 => {
                // Each connection's points on the two profiles: corners, or points along edges, which become corners
                // of their polygons (batch 187 c001–c003: a square's edge split at its middle, a fifth corner).
                let mut pairs = Vec::new();
                let mut added: [Vec<Vec3>; 2] = [Vec::new(), Vec::new()];
                for connection in field(definition, "connections").as_array().map(|a| a.to_vec()).unwrap_or_default() {
                    let entities: Vec<Shape> = query::evaluate(model, field(&connection, "connectionEntities"))?
                        .into_iter()
                        .map(|r| model.shape(r).clone())
                        .collect();
                    let params: Vec<f64> = field(&connection, "connectionEdgeParameters")
                        .as_array()
                        .map(|a| a.iter().filter_map(Value::as_number).collect())
                        .unwrap_or_default();
                    let edges: Vec<Shape> = query::evaluate(model, field(&connection, "connectionEdgeQueries"))?
                        .into_iter()
                        .filter(|r| r.kind == EntityType::Edge)
                        .map(|r| model.shape(r).clone())
                        .collect();
                    let mut at = [None, None];
                    for (w, wire) in wires.iter().enumerate() {
                        let Some(e) = entities.iter().find(|e| touches(e, wire)) else { continue };
                        at[w] = match e.vertex_point() {
                            Ok(p) => Some(p),
                            Err(_) => match edges.iter().zip(&params).find(|(x, _)| touches(x, wire)) {
                                Some((x, &t)) => {
                                    let p = x.edge_tangent(t, true).map_err(occt_error)?.origin;
                                    added[w].push(p);
                                    Some(p)
                                }
                                None => None,
                            },
                        };
                    }
                    match at {
                        [Some(a), Some(b)] => pairs.push((a, b)),
                        _ => return Err(regen_error("LOFT_CONNECTION_MATCHING")),
                    }
                }
                let grown: Vec<Shape> = (0..2)
                    .map(|w| with_corners(&wires[w], &added[w]))
                    .collect::<R<Option<Vec<Shape>>>>()?
                    .ok_or_else(|| error("@opLoft: connections other than between polygons aren't supported yet"))?;
                match unequal_polygons(&grown, &pairs)? {
                    Some(m) => Some(m),
                    None => matched_polygons_with(&grown, pairs.first().copied())?,
                }
            }
            None => None,
        };
        let Some(matched) = matched else {
            return Err(error("@opLoft: connections other than between two polygons aren't supported yet"));
        };
        if ends.iter().any(Option::is_some) {
            return Err(error("@opLoft: connections with end conditions aren't supported yet"));
        }
        let op = occt::loft_as_given(&matched, solid).map_err(|_| regen_error("LOFT_FAILED"))?;
        add_lofted(model, &id, &op, solid, &wires)?;
        return Ok(Value::UNDEFINED);
    }
    // Each guide meets every profile (074 c005: guides missing them are `LOFT_GUIDE_PROFILE_INTERSECTION`).
    if has_guides {
        for q in field(definition, "guideSubqueries").as_array().map(|a| a.to_vec()).unwrap_or_default() {
            let edges: Vec<Shape> =
                query::evaluate(model, &q)?.into_iter().filter(|r| r.kind == EntityType::Edge).map(|r| model.shape(r).clone()).collect();
            if !wires.iter().all(|w| edges.iter().any(|e| touches(e, w))) {
                return Err(regen_error("LOFT_GUIDE_PROFILE_INTERSECTION"));
            }
        }
    }
    // Sections added along a straight spine between two circles bend the loft between them (batch 112 c004–c006).
    if values::opt_bool(definition, "addSections")?.unwrap_or(false)
        && !field(definition, "spine").is_undefined()
        && !has_guides
        && ends.iter().all(Option::is_none)
    {
        let sections = field(definition, "sectionCount").as_number().unwrap_or(0.0).max(0.0) as usize;
        if let Some((shape, first, last)) = sectioned_circle_loft(&wires, sections, solid)? {
            add_lofted_shape(model, &id, &shape, &first, &last, solid, &wires)?;
            return Ok(Value::UNDEFINED);
        }
    }
    // A smooth profile (a slot, an ellipse) to a circle: one periodic side (batch 187 c012, c013).
    if wires.len() == 2 && ends.iter().all(Option::is_none) && !has_guides {
        for circle_at in 0..2 {
            if let Some((shape, first, last)) = smooth_loft_to_circle(&wires, circle_at, &[], solid)? {
                add_lofted_shape(model, &id, &shape, &first, &last, solid, &wires)?;
                return Ok(Value::UNDEFINED);
            }
        }
    }
    match_circles_to_corners(&mut wires)?;
    // Through more than two profiles, or with conditions at the ends, the engine builds the sides across the
    // profiles itself (natural cubic splines at chord-length parameters, as Onshape's).
    let op = if ends.iter().any(Option::is_some) || wires.len() > 2 {
        let op = occt::loft_clamped(&wires, solid, ends[0].clone(), ends[1].clone())
            .map_err(|_| regen_error("LOFT_FAILED"))?;
        // Tangent to a profile, a side leaves each corner along the chord to its partner projected into the plane.
        // Onshape builds it where that lies strictly within the corner's angle or the angle opposite it, and fails
        // otherwise (batch 061 c011, 108: triangles turned to each other, tangent at either end at any magnitude:
        // a chord along a corner's edge at the start, chords off both angles at the end; turned squares build).
        for (end, wire) in [(&ends[0], &wires[0]), (&ends[1], &wires[wires.len() - 1])] {
            let Some(e) = end.as_ref().filter(|e| e.tangent && e.faces.is_empty()) else { continue };
            if !tangent_corners_fit(&op.shape(), wire, e.vector) {
                return Err(regen_error("LOFT_FAILED"));
            }
        }
        op
    } else if let Some(matched) = matched_polygons(&wires)? {
        occt::loft_as_given(&matched, solid).map_err(|_| regen_error("LOFT_FAILED"))?
    } else if let Some(matched) = unequal_polygons(&wires, &[])? {
        occt::loft_as_given(&matched, solid).map_err(|_| regen_error("LOFT_FAILED"))?
    } else {
        occt::loft(&wires, solid).map_err(|_| regen_error("LOFT_FAILED"))?
    };
    // Guides lying on the loft as it is change nothing (straight ones along a square loft's corners, batch 075
    // c004); one crossing a side between two polygons bends that side through it (075 c005, 112 c001–c003).
    if has_guides {
        let result = op.shape();
        let mut off = Vec::new();
        for q in field(definition, "guideSubqueries").as_array().map(|a| a.to_vec()).unwrap_or_default() {
            for r in query::evaluate(model, &q)?.into_iter().filter(|r| r.kind == EntityType::Edge) {
                let edge = model.shape(r).clone();
                let on = (0..=8).all(|k| {
                    edge.edge_tangent(k as f64 / 8.0, false)
                        .ok()
                        .and_then(|a| occt::vertex(a.origin).ok())
                        .is_some_and(|v| v.distance(&result).is_ok_and(|d| d.distance < 1e-6))
                });
                if !on {
                    off.push(edge);
                }
            }
        }
        if !off.is_empty() {
            let clamped = ends.iter().any(Option::is_some);
            let Some((shape, first, last)) = guided_loft(&op, &wires, &off, solid, clamped)? else {
                return Err(error(if clamped {
                    "@opLoft: guides that change a loft with end conditions this way aren't supported yet"
                } else {
                    "@opLoft: guides that change the loft this way aren't supported yet"
                }));
            };
            let before = model.bodies.len();
            add_lofted_shape(model, &id, &shape, &first, &last, solid, &wires)?;
            // A corner along a guide is no B-spline to std (batch 174 c005).
            let along_guide = |e: &Shape| {
                e.curve_type().ok() != Some(occt::CurveType::Line)
                    && e.edge_tangent(0.5, false).ok().and_then(|a| occt::vertex(a.origin).ok()).is_some_and(|v| {
                        off.iter().any(|g| v.distance(g).is_ok_and(|d| d.distance < 1e-7))
                    })
            };
            for b in before..model.bodies.len() {
                model.update_body(b, |body| {
                    for edge in &mut body.edges {
                        if along_guide(&edge.shape) {
                            edge.info = Rc::new(Info { other_curve: true, ..(*edge.info).clone() });
                        }
                    }
                });
            }
            return Ok(Value::UNDEFINED);
        }
    }
    // A sheet lofted from wires faces the other way from OCCT's, as an extruded one does: outward from a loop
    // (a thin loft thickens outward, batch 028); from an open profile, as Onshape's does.
    let shape = if solid { op.shape() } else { open_loft_facing(op.shape(), open_start, &wires[1]) };
    add_lofted_shape(model, &id, &shape, &op.first(), &op.last(), solid, &wires)?;
    Ok(Value::UNDEFINED)
}

/// Onshape's sheet lofted from an open profile faces along the profile (as its edge runs) crossed with the way to the
/// next profile (batch 144 c003: two arcs, the sheet facing their centres; its sheet metal lofts face so, batch 017).
/// The loft's shape to add (which reverses a sheet), turned round where it would face the other way.
fn open_loft_facing(shape: Shape, start: Option<(Vec3, Vec3)>, next: &Shape) -> Shape {
    use values::{cross, dot, norm, sub};
    let Some((p, t)) = start else { return shape };
    let Some(towards) = occt::vertex(p).and_then(|v| v.distance(next)).ok().map(|d| sub(d.b, d.a)) else {
        return shape;
    };
    let facing = cross(t, towards);
    if norm(facing) < 1e-12 {
        return shape;
    }
    let near = shape
        .sub_shapes(ShapeKind::Face)
        .into_iter()
        .filter_map(|f| f.distance(&occt::vertex(p).ok()?).ok().map(|d| (d.distance, f)))
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let Some((_, face)) = near else { return shape };
    let normal = face.face_fraction(p).and_then(|(u, v)| face.face_tangent_plane(u, v)).map(|f| f.normal);
    // Added reversed, the sheet will face against this normal.
    match normal {
        Ok(n) if dot(n, facing) > 0.0 => shape.reversed(),
        _ => shape,
    }
}

/// The cubic B-spline basis functions on a knot vector at `u`.
fn cubic_basis(knots: &[f64], u: f64) -> Vec<f64> {
    let n = knots.len() - 4;
    let last = knots[n];
    let u = if u >= last { last - 1e-12 } else { u };
    let mut b: Vec<f64> = (0..knots.len() - 1).map(|i| if knots[i] <= u && u < knots[i + 1] { 1.0 } else { 0.0 }).collect();
    for d in 1..=3 {
        b = (0..b.len() - 1)
            .map(|i| {
                let left = if knots[i + d] > knots[i] { (u - knots[i]) / (knots[i + d] - knots[i]) * b[i] } else { 0.0 };
                let right = if knots[i + d + 1] > knots[i + 1] {
                    (knots[i + d + 1] - u) / (knots[i + d + 1] - knots[i + 1]) * b[i + 1]
                } else {
                    0.0
                };
                left + right
            })
            .collect();
    }
    b.truncate(n);
    b
}

/// A loft between two polygons with guides (batches 075 c005, 112 c001–c003, 174 c001–c005), or between coaxial
/// circles with a guide in a plane through their axis (174 c007: the guide revolved, a torus for an arc).
///
/// A guide crossing a side, from a point on its bottom edge to one on its top edge, bends that side alone; the rest of
/// the loft stays as it was. Across the side, a cubic with a knot `t` where the guide crosses: the mean of its
/// fractions along the bottom and top edges. The bottom and top edges are each their straight line with its knot at
/// the guide's end, each part either side keeping its own speed and the knot the two speeds' geometric mean (174
/// c003, to 1e-6); the poles across are ruled between theirs, the two about the knot moved by the guide's offset
/// from its chord, so that the surface passes through the guide (taken at its own parameter) and keeps the ruled
/// side's slope across it. The knot is doubled, but for a guide crossing both edges at their middles, where
/// Onshape's surface is C2 and keeps it single: the middle pole moved by the offset over its basis function there.
/// A guide along a corner (from a corner of one polygon to the corner the loft's side edge joins it to) moves both
/// sides there the same way: across each (as its bottom edge runs), one cubic span, its two poles at the guide's end
/// moved by the offset. Along the sides,
/// each row is a C1 cubic through even stations, as many as Onshape fits the guide with ([`guide_spans`]). The new sides are sewn in
/// place of the old. With conditions at the ends (`clamped`: the loft's sides bulge along it), a guide crossing a side
/// through the middles of its edges or along a corner bends the side's own surface the same way (174 c006: Onshape's
/// surface is its own fit, the guide overriding the condition where it meets the profile). The loft's shape and its
/// caps, or `None` where the guides don't cross sides so.
fn guided_loft(
    op: &occt::Operation,
    wires: &[Shape],
    guides: &[Shape],
    solid: bool,
    clamped: bool,
) -> R<Option<(Shape, Vec<Shape>, Vec<Shape>)>> {
    use values::{add, dot, norm, scale, sub};
    if wires.len() > 2 {
        return if clamped { Ok(None) } else { threaded_guided_loft(op, wires, guides, solid) };
    }
    if wires.len() != 2 {
        return Ok(None);
    }
    if !clamped && let Some(revolved) = revolved_guide_loft(wires, guides, solid)? {
        return Ok(Some(revolved));
    }
    let shape = op.shape();
    let (first, last) = (op.first(), op.last());
    let caps: Vec<Shape> = first.iter().chain(&last).cloned().collect();
    let near = |a: Vec3, b: Vec3| norm(sub(a, b)) < 1e-7;
    let at = |e: &Shape, f: f64| -> R<Vec3> { Ok(e.edge_tangent(f, false).map_err(occt_error)?.origin) };
    let on = |s: &Shape, p: Vec3| occt::vertex(p).ok().and_then(|v| v.distance(s).ok()).is_some_and(|d| d.distance < 1e-7);
    let lerp = |a: Vec3, b: Vec3, t: f64| add(a, scale(sub(b, a), t));
    // Straight edges (a loft's sides are straight B-splines).
    let straight = |e: &Shape| -> bool {
        let (Ok(p), Ok(q)) = (at(e, 0.0), at(e, 1.0)) else { return false };
        [0.25, 0.5, 0.75].iter().all(|&f| at(e, f).is_ok_and(|m| norm(sub(m, lerp(p, q, f))) < 1e-9))
    };
    // A side's corners: its bottom edge from `a` to `b` on the first profile, its top edge from `a2` to `b2` (a's and
    // b's partners along its side edges) on the second.
    let corners = |face: &Shape| -> R<Option<[Vec3; 4]>> {
        let edges = face.sub_shapes(ShapeKind::Edge);
        // With end conditions the side edges bulge; the profiles' edges stay straight.
        let profile_edge = |e: &Shape| at(e, 0.5).is_ok_and(|m| on(&wires[0], m) || on(&wires[1], m));
        if edges.len() != 4 || !edges.iter().all(|e| straight(e) || (clamped && !profile_edge(e))) {
            return Ok(None);
        }
        let (mut bottom, mut top, mut sides) = (None, None, Vec::new());
        for e in &edges {
            let m = at(e, 0.5)?;
            if on(&wires[0], m) {
                bottom = Some(e.clone());
            } else if on(&wires[1], m) {
                top = Some(e.clone());
            } else {
                sides.push(e.clone());
            }
        }
        let (Some(bottom), Some(top), [s0, s1]) = (bottom, top, &sides[..]) else { return Ok(None) };
        let (a, b) = (at(&bottom, 0.0)?, at(&bottom, 1.0)?);
        let partner = |p: Vec3| -> R<Option<Vec3>> {
            for s in [s0, s1] {
                let (x, y) = (at(s, 0.0)?, at(s, 1.0)?);
                if near(x, p) {
                    return Ok(Some(y));
                }
                if near(y, p) {
                    return Ok(Some(x));
                }
            }
            Ok(None)
        };
        let (Some(a2), Some(b2)) = (partner(a)?, partner(b)?) else { return Ok(None) };
        if !(on(&top, a2) && on(&top, b2)) {
            return Ok(None);
        }
        Ok(Some([a, b, a2, b2]))
    };
    // The side as it is, at fractions across (from `a` to `b`) and along (from the first profile): bilinear between
    // its corners, or with end conditions its own surface, whichever way its parameters run.
    type Plain = Rc<dyn Fn(f64, f64) -> R<Vec3>>;
    let plain_side = |face: &Shape, [a, b, a2, b2]: [Vec3; 4]| -> R<Option<Plain>> {
        if !clamped {
            return Ok(Some(Rc::new(move |u: f64, v: f64| Ok(lerp(lerp(a, b, u), lerp(a2, b2, u), v)))));
        }
        let point = |fu: f64, fv: f64| -> R<Vec3> { Ok(face.face_tangent_plane(fu, fv).map_err(occt_error)?.origin) };
        for swap in [false, true] {
            for (flip_u, flip_v) in [(false, false), (true, false), (false, true), (true, true)] {
                let map = move |u: f64, v: f64| {
                    let (x, y) = if swap { (v, u) } else { (u, v) };
                    (if flip_u { 1.0 - x } else { x }, if flip_v { 1.0 - y } else { y })
                };
                let fits = |u: f64, v: f64, p: Vec3| -> R<bool> {
                    let (fu, fv) = map(u, v);
                    Ok(near(point(fu, fv)?, p))
                };
                if fits(0.0, 0.0, a)? && fits(1.0, 0.0, b)? && fits(0.0, 1.0, a2)? {
                    let face = face.clone();
                    return Ok(Some(Rc::new(move |u: f64, v: f64| {
                        let (fu, fv) = map(u, v);
                        Ok(face.face_tangent_plane(fu, fv).map_err(occt_error)?.origin)
                    })));
                }
            }
        }
        Ok(None)
    };
    let sides: Vec<Shape> = shape.sub_shapes(ShapeKind::Face).into_iter().filter(|f| !caps.iter().any(|c| c.is_same(f))).collect();
    let mut replaced: Vec<(Shape, Shape)> = Vec::new();
    for guide in guides {
        let (g0, g1) = (at(guide, 0.0)?, at(guide, 1.0)?);
        let reversed = if on(&wires[0], g0) && on(&wires[1], g1) {
            false
        } else if on(&wires[1], g0) && on(&wires[0], g1) {
            true
        } else {
            return Ok(None);
        };
        let (bottom_point, top_point) = if reversed { (g1, g0) } else { (g0, g1) };
        let guide_at = |v: f64| at(guide, if reversed { 1.0 - v } else { v });
        let faces: Vec<&Shape> = sides.iter().filter(|f| on(f, bottom_point) && on(f, top_point)).collect();
        // (Side, its new surface's rows across at a fraction along, its knots and degree across.)
        let mut built: Vec<(Shape, Box<dyn Fn(f64) -> R<Vec<Vec3>> + '_>, Vec<f64>)> = Vec::new();
        match faces[..] {
            [face] => {
                let Some(corner_points @ [a, b, a2, b2]) = corners(face)? else { return Ok(None) };
                let fraction = |p: Vec3, from: Vec3, to: Vec3| dot(sub(p, from), sub(to, from)) / dot(sub(to, from), sub(to, from));
                let (tb, tt) = (fraction(bottom_point, a, b), fraction(top_point, a2, b2));
                if [tb, tt].iter().any(|&f| f <= 1e-6 || f >= 1.0 - 1e-6) {
                    return Ok(None);
                }
                let Some(plain) = plain_side(face, corner_points)? else { return Ok(None) };
                if (tb - 0.5).abs() < 1e-9 && (tt - 0.5).abs() < 1e-9 {
                    let t = 0.5;
                    let knots = vec![0.0, 0.0, 0.0, 0.0, t, 1.0, 1.0, 1.0, 1.0];
                    let greville = [0.0, t / 3.0, (1.0 + t) / 3.0, (2.0 + t) / 3.0, 1.0];
                    let middle = cubic_basis(&knots, t)[2];
                    let rows = move |v: f64| -> R<Vec<Vec3>> {
                        let bend = scale(sub(guide_at(v)?, plain(t, v)?), 1.0 / middle);
                        greville
                            .iter()
                            .enumerate()
                            .map(|(i, &u)| Ok(if i == 2 { add(plain(u, v)?, bend) } else { plain(u, v)? }))
                            .collect()
                    };
                    built.push((face.clone(), Box::new(rows), knots));
                } else if clamped {
                    return Ok(None);
                } else {
                    let t = (tb + tt) / 2.0;
                    let knots = vec![0.0, 0.0, 0.0, 0.0, t, t, 1.0, 1.0, 1.0, 1.0];
                    // The bottom and top edges' poles: each straight edge, its knot at the guide's end.
                    let edge = move |from: Vec3, to: Vec3, tau: f64| -> [Vec3; 6] {
                        let (fast, slow) = (tau / t, (1.0 - tau) / (1.0 - t));
                        let k = (fast * slow).sqrt();
                        [0.0, tau / 3.0, tau - k * t / 3.0, tau + k * (1.0 - t) / 3.0, 1.0 - (1.0 - tau) / 3.0, 1.0]
                            .map(|f| lerp(from, to, f))
                    };
                    let (low, high) = (edge(a, b, tb), edge(a2, b2, tt));
                    let rows = move |v: f64| -> R<Vec<Vec3>> {
                        // Ruled between them, the two about the knot moved by the guide's offset from its chord.
                        let offset = sub(guide_at(v)?, lerp(bottom_point, top_point, v));
                        Ok((0..6)
                            .map(|i| {
                                let p = lerp(low[i], high[i], v);
                                if i == 2 || i == 3 { add(p, offset) } else { p }
                            })
                            .collect())
                    };
                    built.push((face.clone(), Box::new(rows), knots));
                }
            }
            [f0, f1] => {
                // Along a corner: both sides meeting at the side edge from one end of the guide to the other.
                for face in [f0, f1] {
                    let Some(corner_points @ [a, b, a2, b2]) = corners(face)? else { return Ok(None) };
                    // Across as the bottom edge runs, the guide at its start or its end (174 c005: both).
                    let at_start = if near(a, bottom_point) && near(a2, top_point) {
                        true
                    } else if near(b, bottom_point) && near(b2, top_point) {
                        false
                    } else {
                        return Ok(None);
                    };
                    let Some(plain) = plain_side(face, corner_points)? else { return Ok(None) };
                    let rows = move |v: f64| -> R<Vec<Vec3>> {
                        let offset = sub(guide_at(v)?, plain(if at_start { 0.0 } else { 1.0 }, v)?);
                        [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]
                            .iter()
                            .enumerate()
                            .map(|(i, &u)| Ok(if (i < 2) == at_start { add(plain(u, v)?, offset) } else { plain(u, v)? }))
                            .collect()
                    };
                    built.push((face.clone(), Box::new(rows), vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]));
                }
            }
            _ => return Ok(None),
        }
        let spans = guide_spans(guide, 0.0, 1.0)?;
        for (face, rows, knots_u) in built {
            if replaced.iter().any(|(old, _)| old.is_same(&face)) {
                return Ok(None);
            }
            let params: Vec<f64> = (0..=spans).map(|k| k as f64 / spans as f64).collect();
            let bent = bent_side(&face, &knots_u, &params, rows)?;
            replaced.push((face, bent));
        }
    }
    sew_replaced(&shape, &first, &last, &replaced, solid).map(Some)
}

/// A side's new surface from its rows across (at fractions along, through `params`), facing as the side it replaces.
fn bent_side(face: &Shape, knots_u: &[f64], params: &[f64], rows: impl Fn(f64) -> R<Vec<Vec3>>) -> R<Shape> {
    let count = knots_u.len() - 4;
    let curves = super::surface::sampled_rows_at(count, params, rows)?;
    let nv = curves[0].poles.len();
    let poles: Vec<Vec3> = curves.iter().flat_map(|c| c.poles.iter().copied()).collect();
    let bent = occt::bspline_surface_face(&poles, (count, nv), knots_u, &curves[0].knots, (3, curves[0].degree))
        .map_err(|_| regen_error("LOFT_FAILED"))?;
    // Facing as the side it replaces, compared near its start where the two nearly agree.
    let normal = |f: &Shape, u: f64, v: f64| f.face_tangent_plane(u, v).ok().map(|p| p.normal);
    let old = face.face_inner_point().ok().and_then(|p| face.face_fraction(p).ok()).and_then(|(u, v)| normal(face, u, v));
    let new = normal(&bent, 0.5, 0.02);
    Ok(match (old, new) {
        (Some(x), Some(y)) if values::dot(x, y) < 0.0 => bent.reversed(),
        _ => bent,
    })
}

/// A loft's faces with some sides replaced, sewn (into a solid if `solid`): its shape and its caps, found again.
fn sew_replaced(
    shape: &Shape,
    first: &[Shape],
    last: &[Shape],
    replaced: &[(Shape, Shape)],
    solid: bool,
) -> R<(Shape, Vec<Shape>, Vec<Shape>)> {
    let on = |s: &Shape, p: Vec3| occt::vertex(p).ok().and_then(|v| v.distance(s).ok()).is_some_and(|d| d.distance < 1e-7);
    let faces: Vec<Shape> = shape
        .sub_shapes(ShapeKind::Face)
        .into_iter()
        .filter(|f| !replaced.iter().any(|(old, _)| old.is_same(f)))
        .chain(replaced.iter().map(|(_, new)| new.clone()))
        .collect();
    let sewn = occt::sew_faces(&faces, 1e-6).map_err(|_| regen_error("LOFT_FAILED"))?;
    let result = if solid { occt::solid_from(&sewn).map_err(|_| regen_error("LOFT_FAILED"))? } else { sewn };
    // The caps, found again: sewing makes new faces of them.
    let same_place = |cap: &Shape, f: &Shape| {
        cap.face_inner_point().is_ok_and(|p| on(f, p))
            && f.surface_type().ok() == Some(occt::SurfaceType::Plane)
            && (cap.area().unwrap_or(0.0) - f.area().unwrap_or(-1.0)).abs() < 1e-12
    };
    let find = |caps: &[Shape]| -> Vec<Shape> {
        result.sub_shapes(ShapeKind::Face).into_iter().filter(|f| caps.iter().any(|c| same_place(c, f))).collect()
    };
    Ok((result.clone(), find(first), find(last)))
}

/// How many even spans Onshape fits a guide's stretch (from fraction `g0` to `g1` of its parameter) with along a
/// loft: the fewest, a power of two, whose C1 cubic through the guide's points and tangents at their ends keeps
/// within 1e-3 mm of it (batches 112, 174: 8, 4 and 2 along arcs of 111°, 73° and 22°, 1.1e-4 to 4.7e-4 mm off,
/// where half as many would be 1.8e-3 to 7.5e-3 off; 2 and 4 along a 102° arc's stretches of 46° and 56° between
/// three profiles, 8.7e-4 and 1.2e-4 off).
fn guide_spans(guide: &Shape, g0: f64, g1: f64) -> R<usize> {
    use values::{add, norm, scale, sub};
    let at = |f: f64| -> R<Vec3> { Ok(guide.edge_tangent(f.clamp(0.0, 1.0), false).map_err(occt_error)?.origin) };
    let slope = |f: f64| -> R<Vec3> {
        let h = 1e-6;
        let (lo, hi) = ((f - h).max(0.0), (f + h).min(1.0));
        Ok(scale(sub(at(hi)?, at(lo)?), 1.0 / (hi - lo)))
    };
    const TOLERANCE: f64 = 1e-6;
    let mut n = 1;
    while n < 64 {
        let mut worst: f64 = 0.0;
        for k in 0..n {
            let (a, b) = (g0 + (g1 - g0) * k as f64 / n as f64, g0 + (g1 - g0) * (k + 1) as f64 / n as f64);
            let span = b - a;
            let (pa, pb) = (at(a)?, at(b)?);
            let poles = [pa, add(pa, scale(slope(a)?, span / 3.0)), sub(pb, scale(slope(b)?, span / 3.0)), pb];
            for s in [0.25_f64, 0.5, 0.75] {
                let w = [(1.0 - s).powi(3), 3.0 * (1.0 - s).powi(2) * s, 3.0 * (1.0 - s) * s * s, s.powi(3)];
                let h = (1..4).fold(scale(poles[0], w[0]), |acc, i| add(acc, scale(poles[i], w[i])));
                worst = worst.max(norm(sub(h, at(a + s * span)?)));
            }
        }
        if worst <= TOLERANCE {
            break;
        }
        n *= 2;
    }
    Ok(n)
}

/// A loft through more than two polygons with guides each crossing a side through the middles of all the profiles'
/// edges there (batch 174 c008: squares of 20, 25 and 10 mm): as between two ([`guided_loft`]'s single knot), over
/// the plain loft's side, straight across between its side edges, the guide's parameter carried along the loft
/// evenly between where it meets each profile (Onshape's surface passes through the guide's middle-profile point at
/// the loft's knot there); the rows fitted along each stretch between profiles apart ([`guide_spans`]).
fn threaded_guided_loft(
    op: &occt::Operation,
    wires: &[Shape],
    guides: &[Shape],
    solid: bool,
) -> R<Option<(Shape, Vec<Shape>, Vec<Shape>)>> {
    use values::{add, dot, norm, scale, sub};
    let shape = op.shape();
    let (first, last) = (op.first(), op.last());
    let caps: Vec<Shape> = first.iter().chain(&last).cloned().collect();
    let (bottom_wire, top_wire) = (&wires[0], &wires[wires.len() - 1]);
    let near = |a: Vec3, b: Vec3| norm(sub(a, b)) < 1e-7;
    let on = |s: &Shape, p: Vec3| occt::vertex(p).ok().and_then(|v| v.distance(s).ok()).is_some_and(|d| d.distance < 1e-7);
    let at = |e: &Shape, f: f64| -> R<Vec3> { Ok(e.edge_tangent(f, false).map_err(occt_error)?.origin) };
    let lerp = |a: Vec3, b: Vec3, t: f64| add(a, scale(sub(b, a), t));
    // Where an edge meets a profile: the point, and the fraction along the edge.
    let meets = |e: &Shape, w: &Shape| -> R<Option<(Vec3, f64)>> {
        let Ok(d) = e.distance(w) else { return Ok(None) };
        if d.distance > 1e-7 {
            return Ok(None);
        }
        Ok(Some((d.a, e.edge_fraction(d.a, false).map_err(occt_error)?)))
    };
    let sides: Vec<Shape> = shape.sub_shapes(ShapeKind::Face).into_iter().filter(|f| !caps.iter().any(|c| c.is_same(f))).collect();
    let mut replaced: Vec<(Shape, Shape)> = Vec::new();
    for guide in guides {
        let (g0, g1) = (at(guide, 0.0)?, at(guide, 1.0)?);
        let reversed = if on(bottom_wire, g0) && on(top_wire, g1) {
            false
        } else if on(top_wire, g0) && on(bottom_wire, g1) {
            true
        } else {
            return Ok(None);
        };
        let (bottom_point, top_point) = if reversed { (g1, g0) } else { (g0, g1) };
        let faces: Vec<&Shape> = sides.iter().filter(|f| on(f, bottom_point) && on(f, top_point)).collect();
        let [face] = faces[..] else { return Ok(None) };
        // Its bottom and top edges, and its side edges, each run from bottom to top.
        let (mut bottom, mut top, mut rails) = (None, None, Vec::new());
        for e in face.sub_shapes(ShapeKind::Edge) {
            let m = at(&e, 0.5)?;
            if on(bottom_wire, m) {
                bottom = Some(e);
            } else if on(top_wire, m) {
                top = Some(e);
            } else {
                rails.push(e);
            }
        }
        let (Some(bottom), Some(_top), [r0, r1]) = (bottom, top, &rails[..]) else { return Ok(None) };
        let (a, b) = (at(&bottom, 0.0)?, at(&bottom, 1.0)?);
        let upward = |r: &Shape| -> R<Option<(Shape, bool)>> {
            Ok(if on(bottom_wire, at(r, 0.0)?) { Some((r.clone(), false)) } else if on(bottom_wire, at(r, 1.0)?) { Some((r.clone(), true)) } else { None })
        };
        let (Some(left), Some(right)) = (upward(r0)?, upward(r1)?) else { return Ok(None) };
        let rail = |r: &(Shape, bool), v: f64| -> R<Vec3> { at(&r.0, if r.1 { 1.0 - v } else { v }) };
        let (left, right) = if near(rail(&left, 0.0)?, a) && near(rail(&right, 0.0)?, b) {
            (left, right)
        } else if near(rail(&left, 0.0)?, b) && near(rail(&right, 0.0)?, a) {
            (right, left)
        } else {
            return Ok(None);
        };
        let plain = |u: f64, v: f64| -> R<Vec3> { Ok(lerp(rail(&left, v)?, rail(&right, v)?, u)) };
        // Through every profile at the middle of the side's edge there: the loft's knot (along the side edge) and
        // the guide's fraction where they meet it.
        let mut marks = vec![(0.0, 0.0)];
        for w in &wires[1..wires.len() - 1] {
            let (Some((corner, v)), Some((point, g))) = (meets(&left.0, w)?, meets(guide, w)?) else { return Ok(None) };
            let v = if left.1 { 1.0 - v } else { v };
            let g = if reversed { 1.0 - g } else { g };
            let (other, _) = meets(&right.0, w)?.ok_or_else(|| regen_error("LOFT_FAILED"))?;
            if norm(sub(lerp(corner, other, 0.5), point)) > 1e-7 || near(rail(&left, v)?, corner).then_some(()).is_none() {
                return Ok(None);
            }
            marks.push((v, g));
        }
        marks.push((1.0, 1.0));
        let fraction = |p: Vec3, from: Vec3, to: Vec3| dot(sub(p, from), sub(to, from)) / dot(sub(to, from), sub(to, from));
        if (fraction(bottom_point, a, b) - 0.5).abs() > 1e-9
            || (fraction(top_point, rail(&left, 1.0)?, rail(&right, 1.0)?) - 0.5).abs() > 1e-9
        {
            return Ok(None);
        }
        let carried = |v: f64| -> f64 {
            let k = marks.windows(2).position(|m| v <= m[1].0).unwrap_or(marks.len() - 2);
            let ((v0, g0), (v1, g1)) = (marks[k], marks[k + 1]);
            g0 + (v - v0) / (v1 - v0) * (g1 - g0)
        };
        let guide_at = |v: f64| -> R<Vec3> { let g = carried(v); at(guide, if reversed { 1.0 - g } else { g }) };
        let t = 0.5;
        let knots_u = [0.0, 0.0, 0.0, 0.0, t, 1.0, 1.0, 1.0, 1.0];
        let greville = [0.0, t / 3.0, (1.0 + t) / 3.0, (2.0 + t) / 3.0, 1.0];
        let middle = cubic_basis(&knots_u, t)[2];
        let rows = |v: f64| -> R<Vec<Vec3>> {
            let bend = scale(sub(guide_at(v)?, plain(t, v)?), 1.0 / middle);
            greville.iter().enumerate().map(|(i, &u)| Ok(if i == 2 { add(plain(u, v)?, bend) } else { plain(u, v)? })).collect()
        };
        let mut params = vec![0.0];
        for m in marks.windows(2) {
            let ((v0, g0), (v1, g1)) = (m[0], m[1]);
            let (ga, gb) = if reversed { (1.0 - g1, 1.0 - g0) } else { (g0, g1) };
            let n = guide_spans(guide, ga, gb)?;
            params.extend((1..=n).map(|k| v0 + (v1 - v0) * k as f64 / n as f64));
        }
        if replaced.iter().any(|(old, _)| old.is_same(face)) {
            return Ok(None);
        }
        replaced.push((face.clone(), bent_side(face, &knots_u, &params, rows)?));
    }
    sew_replaced(&shape, &first, &last, &replaced, solid).map(Some)
}

/// A loft between two coaxial circles (in parallel planes square to the line through their centres) with one guide
/// lying in a plane through that line, from one circle to the other: Onshape revolves the guide about the line (batch
/// 174 c007: an arc, a torus), the region between the guide and the line for a solid. Its shape and caps, or `None`.
fn revolved_guide_loft(wires: &[Shape], guides: &[Shape], solid: bool) -> R<Option<(Shape, Vec<Shape>, Vec<Shape>)>> {
    use values::{cross, dot, norm, normalize, scale, sub};
    let [guide] = guides else { return Ok(None) };
    let circle = |w: &Shape| -> Option<occt::Frame> {
        let [edge] = &w.sub_shapes(ShapeKind::Edge)[..] else { return None };
        if edge.curve_type().ok() != Some(occt::CurveType::Circle) {
            return None;
        }
        let p: Vec<Vec3> = [0.0, 0.25, 0.5].iter().filter_map(|t| edge.edge_tangent(*t, false).ok().map(|a| a.origin)).collect();
        let [a, b, c] = p[..] else { return None };
        let n = normalize(cross(sub(b, a), sub(c, a)))?;
        // The centre: equidistant from the three points, in their plane.
        let (ab, ac) = (sub(b, a), sub(c, a));
        let (d1, d2, d12) = (dot(ab, ab), dot(ac, ac), dot(ab, ac));
        let det = 2.0 * (d1 * d2 - d12 * d12);
        if det.abs() < 1e-18 {
            return None;
        }
        let (x, y) = ((d2 * (d1 - d12)) / det, (d1 * (d2 - d12)) / det);
        let centre = values::add(a, values::add(scale(ab, x), scale(ac, y)));
        Some(occt::Frame { origin: centre, normal: n, x: normalize(sub(a, centre))? })
    };
    let (Some(c0), Some(c1)) = (circle(&wires[0]), circle(&wires[1])) else { return Ok(None) };
    let Some(axis) = normalize(sub(c1.origin, c0.origin)) else { return Ok(None) };
    if norm(cross(axis, c0.normal)) > 1e-9 || norm(cross(axis, c1.normal)) > 1e-9 {
        return Ok(None);
    }
    // The guide in one plane through the axis.
    let points: Vec<Vec3> =
        (0..=8).filter_map(|k| guide.edge_tangent(k as f64 / 8.0, false).ok().map(|a| a.origin)).collect();
    let Some(radial) = points.iter().find_map(|p| {
        let v = sub(*p, c0.origin);
        normalize(sub(v, scale(axis, dot(v, axis))))
    }) else {
        return Ok(None);
    };
    let across = cross(axis, radial);
    if points.iter().any(|p| dot(sub(*p, c0.origin), across).abs() > 1e-7) {
        return Ok(None);
    }
    let on = |s: &Shape, p: Vec3| occt::vertex(p).ok().and_then(|v| v.distance(s).ok()).is_some_and(|d| d.distance < 1e-7);
    let (g0, g1) = (points[0], points[points.len() - 1]);
    let (start, end) = if on(&wires[0], g0) && on(&wires[1], g1) {
        (g0, g1)
    } else if on(&wires[1], g0) && on(&wires[0], g1) {
        (g1, g0)
    } else {
        return Ok(None);
    };
    let turn = occt::Axis { origin: c0.origin, direction: axis };
    // An arc guide makes a torus, as Onshape's (a spindle torus where its circle's centre lies across the axis).
    if guide.curve_type().ok() == Some(occt::CurveType::Circle) {
        let circle = guide.curve_definition().map_err(occt_error)?;
        let (r, centre) = (circle.radius1, circle.frame.origin);
        let across_axis = dot(sub(centre, c0.origin), radial);
        let x = if across_axis < 0.0 { scale(radial, -1.0) } else { radial };
        let origin = values::add(c0.origin, scale(axis, dot(sub(centre, c0.origin), axis)));
        let major = across_axis.abs();
        // Round the tube from the guide's start to its end, through its middle.
        let phi = |p: Vec3| {
            let d = sub(p, origin);
            (dot(d, axis) / r).atan2((dot(d, x) - major) / r)
        };
        let middle = guide.edge_tangent(0.5, false).map_err(occt_error)?.origin;
        let (a, m, b) = (phi(start), phi(middle), phi(end));
        let turn = std::f64::consts::TAU;
        let wrap = |angle: f64, from: f64| from + (angle - from).rem_euclid(turn);
        // From a, the way that meets m before b.
        let (lo, hi) = if wrap(m, a) < wrap(b, a) { (a, a + wrap(b, a) - a) } else { (b, b + wrap(a, b) - b) };
        // u = 0 is the guide's own half-plane's circle, so the seam is the guide, as Onshape's circles start there.
        let frame = occt::Frame { origin, normal: axis, x };
        let side = occt::torus_face(frame, major, r, (0.0, turn), (lo, hi)).map_err(|_| regen_error("LOFT_FAILED"))?;
        if !solid {
            return Ok(Some((side.clone(), Vec::new(), Vec::new())));
        }
        // Its two boundary circles (not the seam, the guide, an arc of a circle too).
        let mut circles: Vec<Shape> = Vec::new();
        for e in side.sub_shapes(ShapeKind::Edge) {
            let closed = matches!((e.edge_tangent(0.0, false), e.edge_tangent(1.0, false)), (Ok(p), Ok(q)) if norm(sub(p.origin, q.origin)) < 1e-9);
            if closed && e.curve_type().ok() == Some(occt::CurveType::Circle) && !circles.iter().any(|c| c.is_same(&e)) {
                circles.push(e);
            }
        }
        let mut faces = vec![side];
        for c in &circles {
            faces.push(occt::fill_edges(std::slice::from_ref(c)).map_err(|_| regen_error("LOFT_FAILED"))?);
        }
        let sewn = occt::sew_faces(&faces, 1e-7).map_err(|_| regen_error("LOFT_FAILED"))?;
        let solid_shape = occt::solid_from(&sewn).map_err(|_| regen_error("LOFT_FAILED"))?;
        // Outward, whichever way the torus's parameters run.
        let solid_shape = if solid_shape.volume().map_err(occt_error)? < 0.0 { solid_shape.reversed() } else { solid_shape };
        let cap = |frame: &occt::Frame| -> Vec<Shape> {
            solid_shape
                .sub_shapes(ShapeKind::Face)
                .into_iter()
                .filter(|f| {
                    f.face_plane().is_ok_and(|p| norm(cross(p.normal, axis)) < 1e-9 && dot(sub(p.origin, frame.origin), axis).abs() < 1e-9)
                })
                .collect()
        };
        return Ok(Some((solid_shape.clone(), cap(&c0), cap(&c1))));
    }
    let profile = if solid {
        let edges = [
            occt::line_edge(c0.origin, start).map_err(occt_error)?,
            guide.clone(),
            occt::line_edge(end, c1.origin).map_err(occt_error)?,
            occt::line_edge(c1.origin, c0.origin).map_err(occt_error)?,
        ];
        occt::fill_edges(&edges).map_err(|_| regen_error("LOFT_FAILED"))?
    } else {
        guide.clone()
    };
    let swept = occt::revolve(&profile, turn, 2.0 * std::f64::consts::PI).map_err(|_| regen_error("LOFT_FAILED"))?.shape();
    let swept = if solid { occt::solid_from(&swept).unwrap_or(swept) } else { swept };
    let cap = |frame: &occt::Frame| -> Vec<Shape> {
        swept
            .sub_shapes(ShapeKind::Face)
            .into_iter()
            .filter(|f| f.face_plane().is_ok_and(|p| norm(cross(p.normal, axis)) < 1e-9 && dot(sub(p.origin, frame.origin), axis).abs() < 1e-9))
            .collect()
    };
    Ok(Some((swept.clone(), cap(&c0), cap(&c1))))
}

/// Whether, at each corner of a polygon profile, the loft's side edge leaving it lies strictly within the corner's
/// angle or the angle opposite it, in the profile's plane (normal `normal`). Profiles with curved edges pass.
fn tangent_corners_fit(loft: &Shape, wire: &Shape, normal: Vec3) -> bool {
    let Some(n) = values::normalize(normal) else { return true };
    let profile = wire.sub_shapes(ShapeKind::Edge);
    if profile.iter().any(|e| e.curve_type().ok() != Some(occt::CurveType::Line)) {
        return true;
    }
    let ends_of = |edge: &Shape| -> Option<(Vec3, Vec3)> {
        Some((edge.edge_tangent(0.0, false).ok()?.origin, edge.edge_tangent(1.0, false).ok()?.origin))
    };
    let near = |a: Vec3, b: Vec3| values::norm(values::sub(a, b)) < 1e-7;
    let loft_edges = loft.sub_shapes(ShapeKind::Edge);
    for corner in wire.sub_shapes(ShapeKind::Vertex) {
        let Ok(p) = corner.vertex_point() else { continue };
        // The profile's edges from the corner.
        let sides: Vec<Vec3> = profile
            .iter()
            .filter_map(ends_of)
            .filter_map(|(a, b)| if near(a, p) { Some(b) } else if near(b, p) { Some(a) } else { None })
            .filter_map(|q| values::normalize(values::sub(q, p)))
            .collect();
        let [a, b] = sides[..] else { continue };
        // The side edge leaving the corner out of the profile's plane, its direction there.
        let Some(d) = loft_edges.iter().find_map(|edge| {
            let (s, t) = ends_of(edge)?;
            let (at_start, other) = if near(s, p) { (true, t) } else if near(t, p) { (false, s) } else { return None };
            if values::dot(values::sub(other, p), n).abs() < 1e-7 {
                return None;
            }
            let tangent = edge.edge_tangent(if at_start { 0.0 } else { 1.0 }, false).ok()?.direction;
            let away = if at_start { tangent } else { values::scale(tangent, -1.0) };
            values::normalize(values::sub(away, values::scale(n, values::dot(away, n))))
        }) else {
            continue;
        };
        let turn = |x: Vec3, y: Vec3| values::dot(values::cross(x, y), n);
        let within = |x: Vec3, y: Vec3| {
            let whole = turn(x, y);
            let (first, second) = (turn(x, d), turn(d, y));
            const TOLERANCE: f64 = 1e-9;
            first * whole.signum() > TOLERANCE && second * whole.signum() > TOLERANCE
        };
        if !within(a, b) && !within(values::scale(a, -1.0), values::scale(b, -1.0)) {
            return false;
        }
    }
    true
}

/// Adds a loft's bodies; through profiles with arcs (rational curves), its curved faces are marked
/// (`Info::lofted`).
fn add_lofted(model: &mut Model, id: &FeatureId, op: &occt::Operation, solid: bool, wires: &[Shape]) -> R<()> {
    add_lofted_shape(model, id, &op.shape(), &op.first(), &op.last(), solid, wires)
}

/// As [`add_lofted`], for a loft's shape with its caps given.
fn add_lofted_shape(
    model: &mut Model,
    id: &FeatureId,
    shape: &Shape,
    first: &[Shape],
    last: &[Shape],
    solid: bool,
    wires: &[Shape],
) -> R<()> {
    let before = model.bodies.len();
    add_shape_with_caps(model, id, shape, first, last, solid, !solid)?;
    let rational = wires.iter().flat_map(|w| w.sub_shapes(ShapeKind::Edge)).any(|e| {
        matches!(e.curve_type(), Ok(occt::CurveType::Circle | occt::CurveType::Ellipse))
    });
    if !rational {
        return Ok(());
    }
    for b in before..model.bodies.len() {
        model.update_body(b, |body| {
            for face in &mut body.faces {
                if face.shape.surface_type().ok() != Some(occt::SurfaceType::Plane) {
                    face.info = Rc::new(Info { lofted: true, ..(*face.info).clone() });
                }
            }
        });
    }
    Ok(())
}

/// Two circles in parallel planes lofted along a straight spine with sections added (batches 075 c006, 112
/// c004–c006): Onshape adds `sections` between the profiles at even steps along the loft, each the plain loft's
/// circle there as a cubic through it every 45° from the circles' starts, matching its tangent (so dipping 9.8e-4 of
/// the radius between), and runs a natural cubic spline along the loft through the profiles and those sections at
/// even parameters (its radius every 22.5° between them to 4e-7 of Onshape's, for 3, 5 and 9 sections), its side
/// pole for pole Onshape's: 64 cubic Hermite spans round, periodic, a single knot at each section, its ends the
/// profiles' own circles. The solid's shape and its caps, or `None` where the profiles aren't two such circles
/// started alike, or are equal (a cylinder).
fn sectioned_circle_loft(wires: &[Shape], sections: usize, solid: bool) -> R<Option<(Shape, Vec<Shape>, Vec<Shape>)>> {
    use values::{add, cross, dot, norm, normalize, scale, sub};
    let [w0, w1] = wires else { return Ok(None) };
    if !solid || sections == 0 {
        return Ok(None);
    }
    // Each circle's centre, radius, start direction and the way it runs at its start.
    let circle = |w: &Shape| -> Option<(Vec3, f64, Vec3, Vec3)> {
        let [edge] = &w.sub_shapes(ShapeKind::Edge)[..] else { return None };
        if edge.curve_type().ok() != Some(occt::CurveType::Circle) {
            return None;
        }
        let conic = edge.curve_definition().ok()?;
        let start = edge.edge_tangent(0.0, false).ok()?;
        let x = normalize(sub(start.origin, conic.frame.origin))?;
        Some((conic.frame.origin, conic.radius1, x, normalize(start.direction)?))
    };
    let (Some((c0, r0, x0, y0)), Some((c1, r1, x1, y1))) = (circle(w0), circle(w1)) else { return Ok(None) };
    if norm(sub(x0, x1)) > 1e-9 || norm(sub(y0, y1)) > 1e-9 || dot(cross(x0, y0), sub(c1, c0)).abs() < 1e-12 {
        return Ok(None);
    }
    // Equal circles: Onshape's plain loft is a cylinder, its sections exact circles, so the loft is the plain one
    // (batch 089 c009); its sections are presumably cut from the plain loft's surface, a B-spline otherwise.
    if (r0 - r1).abs() < 1e-12 {
        return Ok(None);
    }
    let (x, y) = (x0, y0);
    let count = sections + 2;
    let h = 1.0 / (count - 1) as f64;
    let quarter = std::f64::consts::FRAC_PI_4;
    // A point of section k (0 and count − 1 the profiles) at angle θ round from the start.
    let point = |k: usize, theta: f64| -> Vec3 {
        let v = k as f64 * h;
        let centre = add(c0, scale(sub(c1, c0), v));
        let r = r0 + (r1 - r0) * v;
        let unit = if k == 0 || k == count - 1 {
            (theta.cos(), theta.sin())
        } else {
            let span = ((theta / quarter).floor() as i64).clamp(0, 7) as f64;
            let (t0, t1) = (span * quarter, (span + 1.0) * quarter);
            let s = (theta - t0) / quarter;
            let p0 = (t0.cos(), t0.sin());
            let p3 = (t1.cos(), t1.sin());
            let k3 = quarter / 3.0;
            let p1 = (p0.0 - k3 * t0.sin(), p0.1 + k3 * t0.cos());
            let p2 = (p3.0 + k3 * t1.sin(), p3.1 - k3 * t1.cos());
            let b = [(1.0 - s).powi(3), 3.0 * (1.0 - s).powi(2) * s, 3.0 * (1.0 - s) * s * s, s.powi(3)];
            (
                b[0] * p0.0 + b[1] * p1.0 + b[2] * p2.0 + b[3] * p3.0,
                b[0] * p0.1 + b[1] * p1.1 + b[2] * p2.1 + b[3] * p3.1,
            )
        };
        add(centre, add(scale(x, r * unit.0), scale(y, r * unit.1)))
    };
    // Along the loft at angle θ: the natural cubic spline through the sections, as C1 cubic spans (its values and
    // slopes at the sections).
    let column = |theta: f64| -> Vec<Vec3> {
        let ys: Vec<Vec3> = (0..count).map(|k| point(k, theta)).collect();
        let n = count - 1;
        // Second derivatives: M₀ = Mₙ = 0, M_{i−1} + 4Mᵢ + M_{i+1} = 6(y_{i+1} − 2yᵢ + y_{i−1})/h².
        let mut m = vec![Vec3::new(0.0, 0.0, 0.0); count];
        if n >= 2 {
            let (mut c, mut d) = (vec![0.0; count], vec![Vec3::new(0.0, 0.0, 0.0); count]);
            for i in 1..n {
                let rhs = scale(add(sub(ys[i + 1], scale(ys[i], 2.0)), ys[i - 1]), 6.0 / (h * h));
                let denominator = 4.0 - if i > 1 { c[i - 1] } else { 0.0 };
                c[i] = 1.0 / denominator;
                d[i] = scale(sub(rhs, if i > 1 { d[i - 1] } else { Vec3::new(0.0, 0.0, 0.0) }), 1.0 / denominator);
            }
            for i in (1..n).rev() {
                m[i] = sub(d[i], scale(m[i + 1], c[i]));
            }
        }
        let slope = |i: usize| -> Vec3 {
            if i < n {
                sub(scale(sub(ys[i + 1], ys[i]), 1.0 / h), scale(add(scale(m[i], 2.0), m[i + 1]), h / 6.0))
            } else {
                add(scale(sub(ys[n], ys[n - 1]), 1.0 / h), scale(add(m[n - 1], scale(m[n], 2.0)), h / 6.0))
            }
        };
        let mut poles = vec![ys[0]];
        for i in 0..n {
            poles.push(add(ys[i], scale(slope(i), h / 3.0)));
            poles.push(sub(ys[i + 1], scale(slope(i + 1), h / 3.0)));
            poles.push(ys[i + 1]);
        }
        // As C1 cubic spans: the inner knots doubled, so each value pole between two spans once.
        let mut out = vec![poles[0]];
        for i in 0..n {
            out.extend_from_slice(&poles[3 * i + 1..3 * i + 3]);
            if i == n - 1 {
                out.push(poles[3 * n]);
            }
        }
        out
    };
    let mut knots_along = vec![0.0; 4];
    for i in 1..count - 1 {
        knots_along.extend([i as f64 * h, i as f64 * h]);
    }
    knots_along.extend([1.0; 4]);
    let rows = column(0.0).len();
    // Round the circles, as Onshape's: 64 cubic Hermite spans, periodic, the slopes the columns' own (each a
    // function of the angle all the way round), u round and v along.
    let tau = 2.0 * std::f64::consts::PI;
    let spans = 64;
    let step = 1e-6;
    let mut grid: Vec<Vec<Vec3>> = Vec::with_capacity(2 * spans + 2);
    let mut values_at = Vec::with_capacity(spans + 1);
    for k in 0..spans {
        let f = k as f64 / spans as f64;
        let (p, a, b) = (column(tau * f), column(tau * (f - step)), column(tau * (f + step)));
        let slope: Vec<Vec3> = a.iter().zip(&b).map(|(&a, &b)| scale(sub(b, a), 1.0 / (2.0 * step))).collect();
        values_at.push((p, slope));
    }
    // All the way round, exactly where it started.
    values_at.push(values_at[0].clone());
    // Each station's value lies midway between the poles either side of its double knot.
    let third = 1.0 / (3.0 * spans as f64);
    let offset = |k: usize, sign: f64| -> Vec<Vec3> {
        let (p, d) = &values_at[k];
        p.iter().zip(d).map(|(&p, &d)| add(p, scale(d, sign * third))).collect()
    };
    grid.push(values_at[0].0.clone());
    grid.push(offset(0, 1.0));
    for k in 1..spans {
        grid.push(offset(k, -1.0));
        grid.push(offset(k, 1.0));
    }
    grid.push(offset(spans, -1.0));
    grid.push(values_at[spans].0.clone());
    let mut knots_round = vec![0.0; 4];
    for k in 1..spans {
        knots_round.extend([k as f64 / spans as f64; 2]);
    }
    knots_round.extend([1.0; 4]);
    let poles: Vec<Vec3> = grid.iter().flat_map(|row| row.iter().copied()).collect();
    let side = occt::u_periodic_bspline_surface_face(&poles, (grid.len(), rows), &knots_round, &knots_along, (3, 3))
        .map_err(|_| regen_error("LOFT_FAILED"))?;
    // The ends: the profiles' own circles, as Onshape's edges are.
    let cap = |w: &Shape| -> R<Shape> {
        occt::fill_edges(&w.sub_shapes(ShapeKind::Edge)).map_err(|_| regen_error("LOFT_FAILED"))
    };
    let (first, last) = (cap(w0)?, cap(w1)?);
    let sewn = occt::sew_faces(&[side, first.clone(), last.clone()], 1e-6).map_err(|_| regen_error("LOFT_FAILED"))?;
    let shape = occt::solid_from(&sewn).map_err(|_| regen_error("LOFT_FAILED"))?;
    let shape = if shape.volume().map_err(occt_error)? < 0.0 { shape.reversed() } else { shape };
    // The caps, found again: sewing makes new faces of them.
    let find = |c: &Shape| -> Vec<Shape> {
        let Ok(p) = c.face_inner_point() else { return Vec::new() };
        shape
            .sub_shapes(ShapeKind::Face)
            .into_iter()
            .filter(|f| f.surface_type().ok() == Some(occt::SurfaceType::Plane))
            .filter(|f| occt::vertex(p).ok().and_then(|v| v.distance(f).ok()).is_some_and(|d| d.distance < 1e-7))
            .collect()
    };
    Ok(Some((shape.clone(), find(&first), find(&last))))
}
