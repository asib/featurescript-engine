//! `@opRevolve`: revolving faces about an axis into solids.

use super::ShapeSet;
use crate::model::{BodyType, Cap, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, field};
use crate::{occt_error, query, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Shape, ShapeKind};
use std::f64::consts::TAU;
use std::rc::Rc;

/// An angle normalized to [0, 2π).
fn normalized(angle: f64) -> f64 {
    let a = angle.rem_euclid(TAU);
    if (TAU - a).abs() < 1e-12 { 0.0 } else { a }
}

pub fn revolve(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let found = query::evaluate(model, field(definition, "entities"))?;
    let faces: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Face).map(|r| model.shape(*r).clone()).collect();
    let edges: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Edge).map(|r| model.shape(*r).clone()).collect();
    if faces.is_empty() && edges.is_empty() {
        return Err(regen_error("REVOLVE_SELECT_FACES"));
    }
    let axis = values::line(field(definition, "axis"), "opRevolve axis")?;
    let up_to = up_to_bound(model, definition, axis)?;
    let (start, sweep) = if up_to.is_some() {
        // Revolved nearly a whole turn, then cut where it meets the bound.
        (0.0, TAU - 1e-4)
    } else {
        sweep_angles(definition)?
    };
    let full = sweep >= TAU - 1e-12;

    // Faces revolve into solids; edges, joined into wires, into sheets.
    let solid = !faces.is_empty();
    let profile = if !solid {
        occt::compound(&occt::connect_edges(&edges).map_err(occt_error)?)
    } else if faces.len() == 1 {
        faces[0].clone()
    } else {
        occt::merge_faces(&faces).map_err(occt_error)?.shape()
    };
    let profile = if start != 0.0 && !full {
        occt::transform(&profile, &rotation(axis, start)).map_err(occt_error)?.shape()
    } else {
        profile
    };
    let op = occt::revolve(&profile, axis, sweep).map_err(|_| regen_error("REVOLVE_FAILED"))?;
    let (mut start_cap, mut end_cap) = (ShapeSet::default(), ShapeSet::default());
    for face in op.first() {
        start_cap.insert_all(&face);
    }
    for face in op.last() {
        end_cap.insert_all(&face);
    }
    let created = Info::new(id.clone());
    let start_info = Rc::new(Info {
        created_by: id.clone(),
        sketch_entity: None,
        cap: Some(Cap::Start),
        dependency: None,
        ..Default::default()
    });
    let end_info = Rc::new(Info {
        created_by: id.clone(),
        sketch_entity: None,
        cap: Some(Cap::End),
        dependency: None,
        ..Default::default()
    });
    let (kind, body_type) =
        if solid { (ShapeKind::Solid, BodyType::Solid) } else { (ShapeKind::Shell, BodyType::Sheet) };
    let pieces = match &up_to {
        None => op.shape().sub_shapes(kind),
        Some(cutter) => {
            // The piece from the profile to where it first meets the bound; its faces on the bound are the end cap.
            let split = occt::split(std::slice::from_ref(&op.shape()), std::slice::from_ref(cutter))
                .map_err(|_| regen_error("REVOLVE_FAILED"))?;
            let kept = super::extrude::pieces_touching(&split.shape(), std::slice::from_ref(&profile), kind);
            for piece in &kept {
                for f in piece.sub_shapes(ShapeKind::Face) {
                    if f.distance(cutter).is_ok_and(|d| d.distance < 1e-7) && super::extrude::on_surface(&f, cutter) {
                        end_cap.insert_all(&f);
                    }
                }
            }
            kept
        }
    };
    if pieces.is_empty() {
        return Err(regen_error("REVOLVE_FAILED"));
    }
    // Cylinders and cones get Onshape's coordinate system.
    let profile_point =
        profile.bounding_box(false).ok().map(|b| values::scale(values::add(b.min, b.max), 0.5)).unwrap_or(axis.origin);
    let special = |s: &Shape| {
        if end_cap.contains(s) {
            Some(end_info.clone())
        } else if start_cap.contains(s) {
            Some(start_info.clone())
        } else if s.kind() == Some(ShapeKind::Face) {
            super::frames::revolved(s, axis, profile_point).map(|frame| {
                Rc::new(Info {
                    created_by: created.created_by.clone(),
                    surface_frame: Some(frame),
                    ..Default::default()
                })
            })
        } else {
            None
        }
    };
    for piece in pieces {
        // A sheet revolved from edges faces along tangent × sweep direction in Onshape (as an extruded one faces
        // along tangent × direction): the reverse of OCCT's surface of revolution. (Batch 006, c018.)
        let piece = if solid { piece } else { piece.reversed() };
        model.add_body(NewBody::new(piece, body_type, created.clone()), &Origins::default(), &created, &special);
    }
    Ok(Value::UNDEFINED)
}

/// For an end bound up to a face, body or vertex, what to cut the revolve with: the face (a planar one as a large
/// plane), the body, or the half plane from the axis through the vertex. Turned by the bound's offset angle.
fn up_to_bound(model: &Model, definition: &Value, axis: occt::Axis) -> R<Option<Shape>> {
    if let Some(start) = field(definition, "startBound").as_str()
        && start != "BLIND"
        && values::opt_bool(definition, "hasStartBound")?.unwrap_or(true)
    {
        return Err(error(format!("@opRevolve: startBound {start} isn't supported yet")));
    }
    let entity = |key: &str| -> R<Option<crate::model::EntityRef>> {
        Ok(query::evaluate(model, field(definition, key))?.into_iter().next())
    };
    let reach = model
        .bodies
        .iter()
        .filter_map(|b| b.shape.bounding_box(false).ok())
        .map(|b| values::norm(values::sub(b.max, b.min)) + values::norm(values::sub(b.min, axis.origin)))
        .fold(1.0, f64::max);
    let cutter = match field(definition, "endBound").as_str() {
        None | Some("BLIND") => return Ok(None),
        Some("UP_TO_SURFACE") => {
            let face =
                entity("endBoundEntityFace")?.ok_or_else(|| regen_error("REVOLVE_SELECT_TERMINATING_SURFACE"))?;
            let face = model.shape(face).clone();
            match face.face_plane() {
                Ok(plane) => {
                    occt::plane_face(plane, -2.0 * reach, 2.0 * reach, -2.0 * reach, 2.0 * reach).map_err(occt_error)?
                }
                Err(_) => face,
            }
        }
        Some("UP_TO_BODY") => {
            let body = entity("endBoundEntityBody")?.ok_or_else(|| regen_error("REVOLVE_SELECT_TERMINATING_BODY"))?;
            model.bodies[body.body].shape.clone()
        }
        Some("UP_TO_VERTEX") => {
            let vertex =
                entity("endBoundEntityVertex")?.ok_or_else(|| regen_error("REVOLVE_SELECT_TERMINATING_VERTEX"))?;
            let at = model.shape(vertex).sub_shapes(ShapeKind::Vertex);
            let at = at.first().and_then(|v| v.vertex_point().ok()).ok_or_else(|| regen_error("REVOLVE_FAILED"))?;
            let rel = values::sub(at, axis.origin);
            let radial = values::sub(rel, values::scale(axis.direction, values::dot(rel, axis.direction)));
            let out = values::normalize(radial).ok_or_else(|| regen_error("REVOLVE_FAILED"))?;
            let normal = values::cross(axis.direction, out);
            let frame = occt::Frame { origin: axis.origin, normal, x: out };
            occt::plane_face(frame, 0.0, 2.0 * reach, -2.0 * reach, 2.0 * reach).map_err(occt_error)?
        }
        Some(other) => return Err(error(format!("@opRevolve: endBound {other} isn't supported yet"))),
    };
    let offset = if values::opt_bool(definition, "endBoundHasOffset")?.unwrap_or(false) {
        let a = values::angle(field(definition, "endBoundOffset"), "opRevolve endBoundOffset")?;
        if values::opt_bool(definition, "endBoundOffsetFlip")?.unwrap_or(false) { a } else { -a }
    } else {
        0.0
    };
    Ok(Some(if offset == 0.0 {
        cutter
    } else {
        occt::transform(&cutter, &rotation(axis, offset)).map_err(occt_error)?.shape()
    }))
}

/// Where the revolve starts (radians from the profile, right-handed about the axis) and how far it sweeps.
///
/// std's revolve feature passes blind bounds as `startBoundAngle` and `endBoundAngle`, both measured forward from
/// the profile: the sweep runs forward from the start to the end, so a symmetric revolve of `a` starts at
/// `2π - a/2`. Older callers pass `angleForward` and `angleBack`: from `-angleBack` to `angleForward`, a full turn
/// when they're equal.
fn sweep_angles(definition: &Value) -> R<(f64, f64)> {
    let angle = |key: &str| -> R<Option<f64>> {
        match field(definition, key) {
            v if v.is_undefined() => Ok(None),
            v => values::angle(v, key).map(Some),
        }
    };
    if let Some(forward) = angle("angleForward")? {
        let forward = normalized(forward);
        let back = normalized(angle("angleBack")?.unwrap_or(0.0));
        let sweep = forward + back;
        if (forward - back).abs() < 1e-12 || sweep >= TAU - 1e-12 {
            return Ok((0.0, TAU));
        }
        return Ok((-back, sweep));
    }
    for key in ["endBound", "startBound"] {
        match field(definition, key).as_str() {
            None | Some("BLIND") => {}
            Some(other) => return Err(error(format!("@opRevolve: {key} {other} isn't supported yet"))),
        }
    }
    let end = angle("endBoundAngle")?.ok_or_else(|| error("@opRevolve: no endBoundAngle"))?;
    let mut start = angle("startBoundAngle")?.unwrap_or(0.0);
    if values::opt_bool(definition, "startOppositeDirection")?.unwrap_or(false) {
        start = -start;
    }
    let mut sweep = (end - start).rem_euclid(TAU);
    if sweep < 1e-12 {
        // Zero or a whole number of turns: a full revolve when the ends differ by a turn.
        sweep = if (end - start).abs() > 1e-12 { TAU } else { 0.0 };
    }
    if sweep == 0.0 {
        return Err(regen_error("REVOLVE_FAILED"));
    }
    Ok((start, sweep))
}

/// A rotation by `angle` radians, right-handed, about an axis.
pub fn rotation(axis: occt::Axis, angle: f64) -> occt::Transform {
    let (x, y, z) = (axis.direction.x, axis.direction.y, axis.direction.z);
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;
    let r = [
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
    ];
    let o = axis.origin;
    let mut m = [0.0; 12];
    for i in 0..3 {
        for j in 0..3 {
            m[i * 4 + j] = r[i][j];
        }
        // The origin stays put: translation = o - R o.
        let ro = r[i][0] * o.x + r[i][1] * o.y + r[i][2] * o.z;
        m[i * 4 + 3] = [o.x, o.y, o.z][i] - ro;
    }
    occt::Transform { m }
}
