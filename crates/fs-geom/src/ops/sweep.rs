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

/// A profile of straight edges swept along a ruling of a cylinder or a cone it is locked to: carried along the line,
/// each edge making a face Onshape gives as a B-spline, linear across the edge and a cubic of evenly spaced poles
/// along the line (batch 070 c008), between the profile and its copy at the far end. `None` if the line isn't a
/// ruling (in a plane through the axis) or the profile has curved edges.
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
    let loops = profile.sub_shapes(ShapeKind::Wire);
    let [outer] = loops.as_slice() else { return Ok(None) };
    let profile_edges = outer.sub_shapes(ShapeKind::Edge);
    if profile_edges.iter().any(|e| e.curve_type().ok() != Some(occt::CurveType::Line)) {
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
    for e in &profile_edges {
        let ends = e.edge_tangent(0.0, false).map_err(occt_error)?.origin;
        let ende = e.edge_tangent(1.0, false).map_err(occt_error)?.origin;
        let mut poles = Vec::new();
        for p in [ends, ende] {
            for k in 0..4 {
                poles.push(add(p, scale(travel, k as f64 / 3.0)));
            }
        }
        sides.push(
            occt::bspline_surface_face(&poles, (2, 4), &[0.0, 0.0, 1.0, 1.0], &[0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0], (1, 3))
                .map_err(occt_error)?,
        );
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
        if faces.len() == 1 { faces } else { vec![occt::merge_faces(&faces).map_err(occt_error)?.shape()] }
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
        // Two polygons connected corner to corner: that matching (batch 075 c001–c003).
        let first = field(definition, "connections").as_array().and_then(|a| a.first().cloned()).unwrap_or(Value::UNDEFINED);
        let entities: Vec<Shape> =
            query::evaluate(model, field(&first, "connectionEntities"))?.into_iter().map(|r| model.shape(r).clone()).collect();
        let point = |w: &Shape| -> Option<Vec3> {
            let e = entities.iter().find(|e| touches(e, w))?;
            e.vertex_point().ok().or_else(|| e.edge_tangent(0.5, false).ok().map(|a| a.origin))
        };
        let forced = match (wires.len(), point(&wires[0]), wires.get(1).and_then(point)) {
            (2, Some(a), Some(b)) => Some((a, b)),
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
        let parameters: Vec<f64> = field(&first, "connectionEdgeParameters")
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_number).collect())
            .unwrap_or_default();
        let connection_edges: Vec<Shape> = query::evaluate(model, field(&first, "connectionEdgeQueries"))?
            .into_iter()
            .filter(|r| r.kind == EntityType::Edge)
            .map(|r| model.shape(r).clone())
            .collect();
        if circles.len() == 2 && wires.len() == 2 && connection_edges.len() == 2 && parameters.len() == 2 {
            use crate::values::{cross, normalize, sub};
            let mut started = Vec::new();
            for (conic, edge) in &circles {
                let k = connection_edges
                    .iter()
                    .position(|e| touches(e, edge))
                    .ok_or_else(|| regen_error("LOFT_CONNECTION_MATCHING"))?;
                let at = connection_edges[k].edge_tangent(parameters[k], true).map_err(occt_error)?.origin;
                // Running as the profile's own circle does.
                let start = edge.edge_tangent(0.0, false).map_err(occt_error)?;
                let normal = normalize(cross(sub(start.origin, conic.frame.origin), start.direction))
                    .ok_or_else(|| regen_error("LOFT_FAILED"))?;
                let x = normalize(sub(at, conic.frame.origin)).ok_or_else(|| regen_error("LOFT_FAILED"))?;
                let circle = occt::circle_edge(occt::Frame { origin: conic.frame.origin, normal, x }, conic.radius1)
                    .map_err(occt_error)?;
                started.push(occt::ordered_wire(&[circle]).map_err(occt_error)?);
            }
            if ends.iter().any(Option::is_some) {
                return Err(error("@opLoft: connections with end conditions aren't supported yet"));
            }
            let op = occt::loft_as_given(&started, solid).map_err(|_| regen_error("LOFT_FAILED"))?;
            add_lofted(model, &id, &op, solid, &wires)?;
            return Ok(Value::UNDEFINED);
        }
        let Some(matched) = forced.map(|f| matched_polygons_with(&wires, Some(f))).transpose()?.flatten() else {
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
/// even parameters (its radius every 22.5° between them to 4e-7 of Onshape's, for 3, 5 and 9 sections). The solid's
/// shape and its caps, or `None` where the profiles aren't two such circles started alike, or are equal (a cylinder).
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
    let round: Vec<f64> = (0..=128).map(|i| i as f64 / 128.0).collect();
    let tau = 2.0 * std::f64::consts::PI;
    let curves = super::surface::sampled_rows_at(rows, &round, |f| Ok(column(tau * f)))?;
    let nv = curves[0].poles.len();
    let poles: Vec<Vec3> = curves.iter().flat_map(|c| c.poles.iter().copied()).collect();
    let side = occt::bspline_surface_face(&poles, (rows, nv), &knots_along, &curves[0].knots, (3, 3))
        .map_err(|_| regen_error("LOFT_FAILED"))?;
    let cap = |c: &occt::BSplineData| -> R<Shape> {
        let edge = occt::bspline_edge(&c.poles, &[], &c.knots, 3, false).map_err(occt_error)?;
        occt::fill_edges(&[edge]).map_err(|_| regen_error("LOFT_FAILED"))
    };
    let (first, last) = (cap(&curves[0])?, cap(&curves[rows - 1])?);
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
