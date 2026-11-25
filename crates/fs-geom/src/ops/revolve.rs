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
    let backward = occt::Axis { origin: axis.origin, direction: values::scale(axis.direction, -1.0) };
    let end_cut = up_to_bound(model, definition, axis, "end")?;
    // A start bound up to something: the revolve runs back from where the end's blind angle puts the profile, to
    // where the bound is first met going forward from the profile (batch 024). Its offset turns the bound forward,
    // as the end's does.
    // (In the end's direction, `startOppositeDirection`, the same: a point 20° on with the end 60° on, the revolve runs
    // from the point to the end, batch 164 c003.)
    let start_cut = if values::opt_bool(definition, "hasStartBound")?.unwrap_or(true) {
        up_to_bound(model, definition, axis, "start")?
    } else {
        None
    };
    // Both bounds up to something: each is where it's first met going forward from the profile, and the revolve runs
    // forward from the start's to the end's (batch 164 c001, c002: up to planes either side, half the ring).
    let both = end_cut.is_some() && start_cut.is_some();
    let (start, sweep) = if end_cut.is_some() {
        // Revolved nearly a whole turn, then cut where it meets the bound.
        (0.0, TAU - 1e-4)
    } else if start_cut.is_some() {
        let end = match field(definition, "endBoundAngle") {
            v if v.is_undefined() => 0.0,
            v => values::angle(v, "opRevolve endBoundAngle")?,
        };
        (end, TAU - 1e-4)
    } else {
        sweep_angles(definition)?
    };
    let up_to = end_cut.clone().or(start_cut.clone());
    let reversed = start_cut.is_some() && !both;
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
    let placed = if start != 0.0 && !full {
        occt::transform(&profile, &rotation(axis, start)).map_err(occt_error)?.shape()
    } else {
        profile.clone()
    };
    let op = occt::revolve(&placed, if reversed { backward } else { axis }, sweep)
        .map_err(|_| regen_error("REVOLVE_FAILED"))?;
    let (mut start_cap, mut end_cap) = (ShapeSet::default(), ShapeSet::default());
    // Run backward, the revolve's first faces are the end and the bound the start.
    let (first_cap, bound_cap) = if reversed { (&mut end_cap, &mut start_cap) } else { (&mut start_cap, &mut end_cap) };
    for face in op.first() {
        first_cap.insert_all(&face);
    }
    if !reversed {
        for face in op.last() {
            bound_cap.insert_all(&face);
        }
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
        Some(end_cutter) if both => {
            let start_cutter = start_cut.as_ref().expect("both bounds");
            let fail = |_| regen_error("REVOLVE_FAILED");
            // From the profile to where the end bound is first met.
            let split = occt::split(std::slice::from_ref(&op.shape()), std::slice::from_ref(end_cutter)).map_err(fail)?;
            let ahead = super::extrude::pieces_touching(&split.shape(), std::slice::from_ref(&profile), kind);
            // The start bound met on the way there: from it to the end bound.
            let cut = occt::split(&ahead, std::slice::from_ref(start_cutter)).map_err(fail)?.shape().sub_shapes(kind);
            let kept = if cut.len() > ahead.len() {
                let first = super::extrude::pieces_touching(&occt::compound(&cut), std::slice::from_ref(&profile), kind);
                cut.into_iter().filter(|p| !first.iter().any(|f| f.is_same(p))).collect::<Vec<_>>()
            } else {
                // Otherwise from where it's first met going forward on round to the profile, as a start bound alone
                // (run back from the profile, everything but the last piece), joined to the way to the end.
                let back = occt::revolve(&profile, backward, TAU - 1e-4).map_err(|_| regen_error("REVOLVE_FAILED"))?;
                let split = occt::split(std::slice::from_ref(&back.shape()), std::slice::from_ref(start_cutter)).map_err(fail)?;
                let tail = super::extrude::pieces_touching(&split.shape(), &back.last(), kind);
                let behind: Vec<Shape> =
                    split.shape().sub_shapes(kind).into_iter().filter(|p| !tail.iter().any(|t| t.is_same(p))).collect();
                let fused = occt::boolean(occt::BooleanKind::Union, &ahead, &behind, true).map_err(fail)?;
                fused.shape().sub_shapes(kind)
            };
            for piece in &kept {
                for f in piece.sub_shapes(ShapeKind::Face) {
                    let on = |c: &Shape| f.distance(c).is_ok_and(|d| d.distance < 1e-7) && super::extrude::on_surface(&f, c);
                    if on(end_cutter) {
                        end_cap.insert_all(&f);
                    } else if on(start_cutter) {
                        start_cap.insert_all(&f);
                    }
                }
            }
            kept
        }
        None => op.shape().sub_shapes(kind),
        Some(cutter) if reversed => {
            // Run back from the end nearly a turn and cut by the bound, everything but the last piece: back to the
            // last place the bound is met, which is the first going forward from the profile.
            let split = occt::split(std::slice::from_ref(&op.shape()), std::slice::from_ref(cutter))
                .map_err(|_| regen_error("REVOLVE_FAILED"))?;
            let tail = super::extrude::pieces_touching(&split.shape(), &op.last(), kind);
            let mut kept: Vec<Shape> =
                split.shape().sub_shapes(kind).into_iter().filter(|p| !tail.iter().any(|t| t.is_same(p))).collect();
            if kept.len() > 1 {
                let fused = occt::boolean(occt::BooleanKind::Union, &kept[..1], &kept[1..], true)
                    .map_err(|_| regen_error("REVOLVE_FAILED"))?;
                kept = fused.shape().sub_shapes(kind);
            }
            let end_faces = op.first();
            for piece in &kept {
                for f in piece.sub_shapes(ShapeKind::Face) {
                    if f.distance(cutter).is_ok_and(|d| d.distance < 1e-7) && super::extrude::on_surface(&f, cutter) {
                        bound_cap.insert_all(&f);
                    } else if end_faces
                        .iter()
                        .any(|e| f.distance(e).is_ok_and(|d| d.distance < 1e-7) && super::extrude::on_surface(&f, e))
                    {
                        first_cap.insert_all(&f);
                    }
                }
            }
            kept
        }
        Some(cutter) => {
            // The piece from the profile to where it first meets the bound; its faces on the bound are the end cap.
            let split = occt::split(std::slice::from_ref(&op.shape()), std::slice::from_ref(cutter))
                .map_err(|_| regen_error("REVOLVE_FAILED"))?;
            let kept = super::extrude::pieces_touching(&split.shape(), std::slice::from_ref(&profile), kind);
            for piece in &kept {
                for f in piece.sub_shapes(ShapeKind::Face) {
                    if f.distance(cutter).is_ok_and(|d| d.distance < 1e-7) && super::extrude::on_surface(&f, cutter) {
                        bound_cap.insert_all(&f);
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
    // OCCT turns the profile anticlockwise about the axis it's given (backward for a start bound).
    let revolve_sign = if reversed { -1.0 } else { 1.0 };
    let special = |s: &Shape| {
        if end_cap.contains(s) {
            Some(end_info.clone())
        } else if start_cap.contains(s) {
            Some(start_info.clone())
        } else if s.kind() == Some(ShapeKind::Edge) && s.curve_type().ok() == Some(occt::CurveType::Circle) {
            // An edge swept from a vertex, round the axis: it runs back against the revolve (batch 088 c007).
            let c = s.curve_definition().ok()?;
            if values::norm(values::cross(c.frame.normal, axis.direction)) > 1e-9 {
                return None;
            }
            let mid = s.edge_tangent(0.5, false).ok()?;
            let radial = values::sub(mid.origin, c.frame.origin);
            let forward = values::normalize(values::cross(values::scale(axis.direction, revolve_sign), radial))?;
            Some(Rc::new(Info {
                created_by: created.created_by.clone(),
                sense: Some((mid.origin, values::scale(forward, -1.0))),
                ..Default::default()
            }))
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

/// For a bound (`which` is "end" or "start") up to a face, body or vertex, what to cut the revolve with: the face (a
/// planar one as a large plane), the body, or the half plane from the axis through the vertex. Turned by the bound's
/// offset angle about `axis`, the way the revolve runs to it.
fn up_to_bound(model: &Model, definition: &Value, axis: occt::Axis, which: &str) -> R<Option<Shape>> {
    let key = |name: &str| format!("{which}{name}");
    let entity = |name: &str| -> R<Option<crate::model::EntityRef>> {
        Ok(query::evaluate(model, field(definition, &key(name)))?.into_iter().next())
    };
    // How far a cutting plane has to reach: found only for a bound that's cut by one, as it boxes every body.
    let reach = || {
        model
            .bodies
            .iter()
            .filter_map(|b| b.shape.bounding_box(false).ok())
            .map(|b| values::norm(values::sub(b.max, b.min)) + values::norm(values::sub(b.min, axis.origin)))
            .fold(1.0, f64::max)
    };
    let cutter = match field(definition, &key("Bound")).as_str() {
        None | Some("BLIND") => return Ok(None),
        Some("UP_TO_SURFACE") => {
            let face = entity("BoundEntityFace")?.ok_or_else(|| regen_error("REVOLVE_SELECT_TERMINATING_SURFACE"))?;
            let face = model.shape(face).clone();
            match face.face_plane() {
                Ok(plane) => {
                    let reach = reach();
                    occt::plane_face(plane, -2.0 * reach, 2.0 * reach, -2.0 * reach, 2.0 * reach).map_err(occt_error)?
                }
                Err(_) => face,
            }
        }
        Some("UP_TO_BODY") => {
            let body = entity("BoundEntityBody")?.ok_or_else(|| regen_error("REVOLVE_SELECT_TERMINATING_BODY"))?;
            model.bodies[body.body].shape.clone()
        }
        Some("UP_TO_VERTEX") => {
            let vertex =
                entity("BoundEntityVertex")?.ok_or_else(|| regen_error("REVOLVE_SELECT_TERMINATING_VERTEX"))?;
            let at = model.shape(vertex).sub_shapes(ShapeKind::Vertex);
            let at = at.first().and_then(|v| v.vertex_point().ok()).ok_or_else(|| regen_error("REVOLVE_FAILED"))?;
            let rel = values::sub(at, axis.origin);
            let radial = values::sub(rel, values::scale(axis.direction, values::dot(rel, axis.direction)));
            let out = values::normalize(radial).ok_or_else(|| regen_error("REVOLVE_FAILED"))?;
            let normal = values::cross(axis.direction, out);
            let frame = occt::Frame { origin: axis.origin, normal, x: out };
            let reach = reach();
            occt::plane_face(frame, 0.0, 2.0 * reach, -2.0 * reach, 2.0 * reach).map_err(occt_error)?
        }
        Some(other) => return Err(error(format!("@opRevolve: {which}Bound {other} isn't supported yet"))),
    };
    let offset = if values::opt_bool(definition, &key("BoundHasOffset"))?.unwrap_or(false) {
        let a = values::angle(field(definition, &key("BoundOffset")), "opRevolve bound offset")?;
        if values::opt_bool(definition, &key("BoundOffsetFlip"))?.unwrap_or(false) { -a } else { a }
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
