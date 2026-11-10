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
    let (mut start_cap, mut end_cap) = (ShapeSet::default(), ShapeSet::default());
    for face in op.first() {
        start_cap.insert_all(&face);
    }
    for face in op.last() {
        end_cap.insert_all(&face);
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
    let mut pieces = op.shape().sub_shapes(kind);
    if pieces.is_empty() && !solid {
        pieces = op.shape().sub_shapes(ShapeKind::Face);
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
    let path: Vec<Shape> = query::evaluate(model, field(definition, "path"))?
        .iter()
        .filter(|r| r.kind == EntityType::Edge)
        .map(|r| model.shape(*r).clone())
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
    // How far round: the whole circle, or the arc from its start to its end about the circle's own normal.
    let ends = edge.edge_end_vertices().map_err(occt_error)?;
    let closed = ends.len() < 2 || ends[0].is_same(&ends[1]);
    let angle = if closed {
        2.0 * std::f64::consts::PI
    } else {
        let a = edge.edge_tangent(0.0, false).map_err(occt_error)?.origin;
        let b = edge.edge_tangent(1.0, false).map_err(occt_error)?.origin;
        let (u, w) = (sub(a, circle.frame.origin), sub(b, circle.frame.origin));
        let mut t = dot(cross(u, w), circle.frame.normal).atan2(dot(u, w));
        if t <= 0.0 {
            t += 2.0 * std::f64::consts::PI;
        }
        t
    };
    let found = query::evaluate(model, field(definition, "profiles"))?;
    let faces: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Face).map(|r| model.shape(*r).clone()).collect();
    if faces.is_empty() {
        return Ok(None);
    }
    for profile in &faces {
        let op =
            occt::revolve(profile, occt::Axis { origin: circle.frame.origin, direction: circle.frame.normal }, angle)
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
                let normals: Vec<Vec3> =
                    faces.iter().map(|f| f.face_plane().map(|p| p.normal)).collect::<Result<_, _>>().map_err(|_| {
                        error("@opSweep: profiles locked to faces other than planes aren't supported yet")
                    })?;
                let Some(&first) = normals.first() else { return Err(regen_error("SWEEP_SELECT_LOCK_FACES")) };
                if normals.iter().any(|n| values::norm(values::cross(*n, first)) > 1e-9) {
                    return Err(error("@opSweep: profiles locked to faces of different planes aren't supported yet"));
                }
                Some(Some(first))
            }
            Some(other) => return Err(error(format!("@opSweep: profileControl {other} isn't supported yet"))),
        }
    };
    let found = query::evaluate(model, field(definition, "profiles"))?;
    let faces: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Face).map(|r| model.shape(*r).clone()).collect();
    let edges: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Edge).map(|r| model.shape(*r).clone()).collect();
    let path_edges: Vec<Shape> = query::evaluate(model, field(definition, "path"))?
        .iter()
        .filter(|r| r.kind == EntityType::Edge)
        .map(|r| model.shape(*r).clone())
        .collect();
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
        let op = match control {
            Some(lock) => occt::sweep_controlled(profile, path, lock),
            None => occt::sweep(profile, path),
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

/// A circle lofted to a profile with corners is split where the rays from its centre through those corners meet it,
/// one arc to each edge, as Onshape splits it (batch 004 c039: a square's corners at 45°, 135°, 225° and 315°).
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
    for q in subqueries.iter() {
        let found = query::evaluate(model, q)?;
        let faces: Vec<&_> = found.iter().filter(|r| r.kind == EntityType::Face).collect();
        if let Some(face) = faces.first() {
            if faces.len() > 1 {
                return Err(error("@opLoft: profiles of several faces aren't supported yet"));
            }
            wires.push(occt::outer_wire(model.shape(**face)).map_err(occt_error)?);
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
        let mut connected = occt::connect_edges(&edges).map_err(occt_error)?;
        if connected.len() != 1 {
            return Err(regen_error("LOFT_PROFILE_FAILED"));
        }
        wires.push(connected.remove(0));
    }
    if wires.len() < 2 {
        return Err(regen_error("LOFT_SELECT_PROFILES"));
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
            add_with_caps_facing(model, &id, &op, solid, !solid)?;
            return Ok(Value::UNDEFINED);
        }
        let Some(matched) = forced.map(|f| matched_polygons_with(&wires, Some(f))).transpose()?.flatten() else {
            return Err(error("@opLoft: connections other than between two polygons aren't supported yet"));
        };
        if ends.iter().any(Option::is_some) {
            return Err(error("@opLoft: connections with end conditions aren't supported yet"));
        }
        let op = occt::loft_as_given(&matched, solid).map_err(|_| regen_error("LOFT_FAILED"))?;
        add_with_caps_facing(model, &id, &op, solid, !solid)?;
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
    match_circles_to_corners(&mut wires)?;
    // Through more than two profiles, or with conditions at the ends, the engine builds the sides across the
    // profiles itself (natural cubic splines at chord-length parameters, as Onshape's).
    let op = if ends.iter().any(Option::is_some) || wires.len() > 2 {
        occt::loft_clamped(&wires, solid, ends[0].clone(), ends[1].clone()).map_err(|_| regen_error("LOFT_FAILED"))?
    } else if let Some(matched) = matched_polygons(&wires)? {
        occt::loft_as_given(&matched, solid).map_err(|_| regen_error("LOFT_FAILED"))?
    } else {
        occt::loft(&wires, solid).map_err(|_| regen_error("LOFT_FAILED"))?
    };
    // Guides lying on the loft as it is change nothing (straight ones along a square loft's corners, batch 075
    // c004); others aren't supported yet.
    if has_guides {
        let result = op.shape();
        for q in field(definition, "guideSubqueries").as_array().map(|a| a.to_vec()).unwrap_or_default() {
            for r in query::evaluate(model, &q)?.into_iter().filter(|r| r.kind == EntityType::Edge) {
                let edge = model.shape(r);
                let on = (0..=8).all(|k| {
                    edge.edge_tangent(k as f64 / 8.0, false)
                        .ok()
                        .and_then(|a| occt::vertex(a.origin).ok())
                        .is_some_and(|v| v.distance(&result).is_ok_and(|d| d.distance < 1e-6))
                });
                if !on {
                    return Err(error("@opLoft: guides that change the loft aren't supported yet"));
                }
            }
        }
    }
    // A sheet lofted from wires faces the other way from OCCT's, as an extruded one does: outward from a loop
    // (a thin loft thickens outward, batch 028).
    add_with_caps_facing(model, &id, &op, solid, !solid)?;
    Ok(Value::UNDEFINED)
}
