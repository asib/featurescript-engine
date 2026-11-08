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
/// both in proportion to the distance along it (batch 019): a loft through copies of the profile carried along the
/// path, a copy every 2.8° of turn (and of arc, along a circular path, batch 048), ruled between the two ends when it
/// only grows along a straight path (a circle makes a cone). The path is straight (lines along one direction) or
/// one circular arc, along which the copies turn with it about its axis.
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
    let unsupported = || error("@opSweep: twist and scale are only supported along straight paths and single arcs yet");
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
    let arc = matches!(path_edges, [e] if e.curve_type().ok() == Some(occt::CurveType::Circle));
    if !arc && path_edges.iter().any(|e| e.curve_type().ok() != Some(occt::CurveType::Line)) {
        return Err(unsupported());
    }
    let mut ends: Vec<(Vec3, Vec3)> = Vec::new();
    for e in path_edges {
        for t in [0.0, 1.0] {
            let a = e.edge_tangent(t, false).map_err(occt_error)?;
            ends.push((a.origin, a.direction));
        }
    }
    for profile in profiles {
        let wire = if profile.kind() == Some(ShapeKind::Face) {
            occt::outer_wire(profile).map_err(occt_error)?
        } else {
            profile.clone()
        };
        let centre = profile.centroid().map_err(occt_error)?;
        // The path's end at the profile, leaving along the path.
        let near = (0..ends.len())
            .min_by(|&a, &b| norm(sub(ends[a].0, centre)).total_cmp(&norm(sub(ends[b].0, centre))))
            .ok_or_else(unsupported)?;
        // The motion carrying the profile a fraction t along the path (rotation, translation), and the path's
        // point and direction there.
        type Motion = Box<dyn Fn(f64) -> ([[f64; 3]; 3], Vec3, Vec3, Vec3)>;
        let (motion, arc_angle): (Motion, f64) = if arc {
            let edge = &path_edges[0];
            let conic = edge.curve_definition().map_err(occt_error)?;
            let c = conic.frame.origin;
            let (start, leaving) = if near == 0 { ends[0] } else { (ends[1].0, values::scale(ends[1].1, -1.0)) };
            let other = if near == 0 { ends[1].0 } else { ends[0].0 };
            let axis = normalize(cross(sub(start, c), leaving)).ok_or_else(unsupported)?;
            let (u, w) = (sub(start, c), sub(other, c));
            let mut angle = dot(cross(u, w), axis).atan2(dot(u, w));
            if angle <= 1e-12 {
                angle += 2.0 * std::f64::consts::PI;
            }
            let motion: Motion = Box::new(move |t: f64| {
                let r = rot3(axis, angle * t);
                let shift = sub(c, apply(&r, c));
                (r, shift, add(apply(&r, start), shift), apply(&r, leaving))
            });
            (motion, angle)
        } else {
            let start = ends[near].0;
            let end = ends
                .iter()
                .map(|e| e.0)
                .max_by(|a, b| norm(sub(*a, start)).total_cmp(&norm(sub(*b, start))))
                .ok_or_else(unsupported)?;
            let run = sub(end, start);
            let (length, d) = (norm(run), normalize(run).ok_or_else(unsupported)?);
            if ends.iter().any(|p| norm(cross(sub(p.0, start), d)) > 1e-9) {
                return Err(unsupported());
            }
            let motion: Motion = Box::new(move |t: f64| {
                let shift = values::scale(d, length * t);
                (identity, shift, add(start, shift), d)
            });
            (motion, 0.0)
        };
        let step = std::f64::consts::PI / 64.0;
        let by_twist = if twist.abs() > 1e-12 { ((twist.abs() / step).ceil() as usize).max(8) } else { 1 };
        let by_arc = if arc { (arc_angle / step).ceil() as usize } else { 1 };
        let steps = by_twist.max(by_arc);
        let mut wires = Vec::new();
        for k in 0..=steps {
            let t = k as f64 / steps as f64;
            let (r, shift, p, tangent) = motion(t);
            // A positive (counterclockwise) twist turns right-handed about the path's direction (batch 019).
            let turn = rot3(tangent, twist * t);
            let s = 1.0 + (scale - 1.0) * t;
            // x goes to p + s turn (r x + shift - p).
            let a = mul(&turn, &r);
            let b = add(p, values::scale(apply(&turn, sub(shift, p)), s));
            let m = [
                s * a[0][0],
                s * a[0][1],
                s * a[0][2],
                b.x,
                s * a[1][0],
                s * a[1][1],
                s * a[1][2],
                b.y,
                s * a[2][0],
                s * a[2][1],
                s * a[2][2],
                b.z,
            ];
            wires.push(occt::transform(&wire, &occt::Transform { m }).map_err(occt_error)?.shape());
        }
        let op = occt::loft(&wires, solid).map_err(|_| regen_error("SWEEP_FAILED"))?;
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
    // Conditions at the ends: a derivative along a vector (normal to the profile's plane, or a direction given).
    let mut ends: [Option<(occt::Vec3, f64)>; 2] = [None, None];
    for info in field(definition, "derivativeInfo").as_array().map(|a| a.to_vec()).unwrap_or_default() {
        if values::opt_bool(&info, "tangentToPlane")?.unwrap_or(false) || !field(&info, "adjacentFaces").is_undefined()
        {
            return Err(error("@opLoft: end conditions other than along a vector aren't supported yet"));
        }
        let index =
            field(&info, "profileIndex").as_number().ok_or_else(|| error("@opLoft: profileIndex must be a number"))?;
        let magnitude = field(&info, "magnitude").as_number().unwrap_or(1.0);
        let vector = values::vec3(field(&info, "vector"), "@opLoft derivative vector")?;
        ends[if index == 0.0 { 0 } else { 1 }] = Some((vector, magnitude));
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
            // A point profile (a vertex): not supported yet.
            return Err(error("@opLoft: point profiles aren't supported yet"));
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
    let op = if ends.iter().any(Option::is_some) {
        if wires.len() != 2 {
            return Err(error("@opLoft: end conditions on lofts through more than two profiles aren't supported yet"));
        }
        occt::loft_clamped(&wires, solid, ends[0], ends[1]).map_err(|_| regen_error("LOFT_FAILED"))?
    } else {
        occt::loft(&wires, solid).map_err(|_| regen_error("LOFT_FAILED"))?
    };
    // A sheet lofted from wires faces the other way from OCCT's, as an extruded one does: outward from a loop
    // (a thin loft thickens outward, batch 028).
    add_with_caps_facing(model, &id, &op, solid, !solid)?;
    Ok(Value::UNDEFINED)
}
