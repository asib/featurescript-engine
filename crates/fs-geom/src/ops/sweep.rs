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

/// A sweep along a straight path whose profile turns about the path (by `twist` over its length) and grows (to
/// `scale` times its size), both in proportion to the distance along it (batch 019): a loft through copies of the
/// profile, ruled between the two ends when it only grows (a circle makes a cone), through a copy every 2.8° when
/// it turns.
fn twisted_or_scaled(
    model: &mut Model,
    id: &FeatureId,
    profiles: &[Shape],
    path_edges: &[Shape],
    twist: f64,
    scale: f64,
    solid: bool,
) -> R<Value> {
    let not_straight = || error("@opSweep: twist and scale are only supported along straight paths yet");
    let mut ends: Vec<Vec3> = Vec::new();
    for e in path_edges {
        if e.curve_type().ok() != Some(occt::CurveType::Line) {
            return Err(not_straight());
        }
        for t in [0.0, 1.0] {
            ends.push(e.edge_tangent(t, false).map_err(occt_error)?.origin);
        }
    }
    for profile in profiles {
        let wire = if profile.kind() == Some(ShapeKind::Face) {
            occt::outer_wire(profile).map_err(occt_error)?
        } else {
            profile.clone()
        };
        let centre = profile.centroid().map_err(occt_error)?;
        // The path's end at the profile, and its far end.
        let start = *ends
            .iter()
            .min_by(|a, b| values::norm(values::sub(**a, centre)).total_cmp(&values::norm(values::sub(**b, centre))))
            .ok_or_else(not_straight)?;
        let end = *ends
            .iter()
            .max_by(|a, b| values::norm(values::sub(**a, start)).total_cmp(&values::norm(values::sub(**b, start))))
            .ok_or_else(not_straight)?;
        let run = values::sub(end, start);
        let length = values::norm(run);
        let d = values::normalize(run).ok_or_else(not_straight)?;
        if ends.iter().any(|p| values::norm(values::cross(values::sub(*p, start), d)) > 1e-9) {
            return Err(not_straight());
        }
        let steps = if twist.abs() > 1e-12 { ((twist.abs() / (std::f64::consts::PI / 64.0)).ceil() as usize).max(8) } else { 1 };
        let mut wires = Vec::new();
        for k in 0..=steps {
            let t = k as f64 / steps as f64;
            // A positive (counterclockwise) twist turns right-handed about the path's direction (batch 019).
            let r = super::rotation(occt::Axis { origin: Vec3::new(0.0, 0.0, 0.0), direction: d }, twist * t).m;
            let s = 1.0 + (scale - 1.0) * t;
            let linear = |v: Vec3| {
                Vec3::new(
                    s * (r[0] * v.x + r[1] * v.y + r[2] * v.z),
                    s * (r[4] * v.x + r[5] * v.y + r[6] * v.z),
                    s * (r[8] * v.x + r[9] * v.y + r[10] * v.z),
                )
            };
            // x goes to start + s R (x - start) + d L t.
            let shift = values::add(values::sub(start, linear(start)), values::scale(d, length * t));
            let m = [
                s * r[0], s * r[1], s * r[2], shift.x,
                s * r[4], s * r[5], s * r[6], shift.y,
                s * r[8], s * r[9], s * r[10], shift.z,
            ];
            wires.push(occt::transform(&wire, &occt::Transform { m }).map_err(occt_error)?.shape());
        }
        let op = occt::loft(&wires, solid).map_err(|_| regen_error("SWEEP_FAILED"))?;
        add_with_caps(model, id, &op, solid)?;
    }
    Ok(Value::UNDEFINED)
}

pub fn loft(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    for key in ["guideSubqueries", "connections", "derivativeInfo"] {
        if field(definition, key).as_array().is_some_and(|a| !a.is_empty()) {
            return Err(error(format!("@opLoft: {key} isn't supported yet")));
        }
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
    let op = occt::loft(&wires, solid).map_err(|_| regen_error("LOFT_FAILED"))?;
    // A sheet lofted from wires faces the other way from OCCT's, as an extruded one does: outward from a loop
    // (a thin loft thickens outward, batch 028).
    add_with_caps_facing(model, &id, &op, solid, !solid)?;
    Ok(Value::UNDEFINED)
}
