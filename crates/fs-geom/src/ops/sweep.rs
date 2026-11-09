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

pub fn sweep(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
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

pub fn loft(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    for key in ["guideSubqueries", "connections"] {
        if field(definition, key).as_array().is_some_and(|a| !a.is_empty()) {
            return Err(error(format!("@opLoft: {key} isn't supported yet")));
        }
    }
    // Conditions at the ends: a derivative along a vector (normal to the profile's plane, or a direction given) or
    // in the plane square to it (tangent to the profile's plane).
    let mut ends: [Option<(occt::Vec3, f64, bool)>; 2] = [None, None];
    for info in field(definition, "derivativeInfo").as_array().map(|a| a.to_vec()).unwrap_or_default() {
        if !field(&info, "adjacentFaces").is_undefined() {
            return Err(error("@opLoft: end conditions matching faces aren't supported yet"));
        }
        let tangent = values::opt_bool(&info, "tangentToPlane")?.unwrap_or(false);
        let index =
            field(&info, "profileIndex").as_number().ok_or_else(|| error("@opLoft: profileIndex must be a number"))?;
        let magnitude = field(&info, "magnitude").as_number().unwrap_or(1.0);
        let vector = values::vec3(field(&info, "vector"), "@opLoft derivative vector")?;
        ends[if index == 0.0 { 0 } else { 1 }] = Some((vector, magnitude, tangent));
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
    // Through more than two profiles, or with conditions at the ends, the engine builds the sides across the
    // profiles itself (natural cubic splines at chord-length parameters, as Onshape's).
    let op = if ends.iter().any(Option::is_some) || wires.len() > 2 {
        occt::loft_clamped(&wires, solid, ends[0], ends[1]).map_err(|_| regen_error("LOFT_FAILED"))?
    } else {
        occt::loft(&wires, solid).map_err(|_| regen_error("LOFT_FAILED"))?
    };
    // A sheet lofted from wires faces the other way from OCCT's, as an extruded one does: outward from a loop
    // (a thin loft thickens outward, batch 028).
    add_with_caps_facing(model, &id, &op, solid, !solid)?;
    Ok(Value::UNDEFINED)
}
