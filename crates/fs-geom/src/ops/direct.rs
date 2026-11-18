//! Direct edits of solids: `@opShell`, `@opDraft`, `@opThicken`, `@opOffsetFace` and `@opDeleteFace`.

use std::rc::Rc;

use super::{ShapeSet, by_body, replace_bodies};
use crate::model::{BodyType, Cap, Entity, EntityRef, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, field};
use crate::{occt_error, query, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Shape, ShapeKind, Vec3};

pub fn shell(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let thickness = values::length(field(definition, "thickness"), "opShell thickness")?;
    let entities = field(definition, "entities");
    let mut results = Vec::new();
    // Faces to remove, by body; bodies selected whole are hollowed.
    let faces = by_body(model, entities, EntityType::Face)?;
    for (b, faces) in &faces {
        let solid = &model.bodies[*b].shape;
        let op = match occt::shell(solid, faces, thickness) {
            Ok(op) => op,
            Err(_) => tube_shell(solid, faces, thickness)?.ok_or_else(|| regen_error("SHELL_FAILED"))?,
        };
        results.push((*b, op));
    }
    let hollow: Vec<usize> = query::evaluate(model, entities)?
        .into_iter()
        .filter(|r| r.kind == EntityType::Body && !faces.iter().any(|(b, _)| *b == r.body))
        .map(|r| r.body)
        .collect();
    for b in hollow {
        let solid = model.bodies[b].shape.clone();
        // A hollow body: the solid less its inward offset (or, outward, its outward offset less the solid).
        let offset = occt::offset(&solid, thickness).map_err(|_| regen_error("SHELL_FAILED"))?;
        let inner = occt::solid_from(&offset).map_err(|_| regen_error("SHELL_FAILED"))?;
        let op = if thickness < 0.0 {
            occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(&solid), &[inner], false)
        } else {
            occt::boolean(occt::BooleanKind::Subtraction, &[inner], std::slice::from_ref(&solid), false)
        }
        .map_err(|_| regen_error("SHELL_FAILED"))?;
        results.push((b, op));
    }
    if results.is_empty() {
        return Err(regen_error("SHELL_SELECT_FACES"));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

pub fn draft(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    // NEUTRAL_PLANE is the older form of a reference surface draft, with the face in `neutralPlane`.
    let pull = values::normalize(values::vec3(field(definition, "pullVec"), "opDraft pullVec")?)
        .ok_or_else(|| error("@opDraft: zero pull direction"))?;
    let angle = values::angle(field(definition, "angle"), "opDraft angle")?;
    let reference_key = match field(definition, "draftType").as_str() {
        None | Some("REFERENCE_SURFACE") => "referenceSurface",
        Some("NEUTRAL_PLANE") => "neutralPlane",
        Some("REFERENCE_ENTITY") => return draft_about_entities(model, id, definition, pull, angle),
        Some(other) => return Err(error(format!("@opDraft: {other} drafts aren't supported yet"))),
    };
    let reference = field(definition, reference_key);
    let neutral = if reference.get("normal").is_some() {
        values::plane(reference, "opDraft referenceSurface")?
    } else {
        let found = query::evaluate(model, reference)?;
        let face =
            found.iter().find(|r| r.kind == EntityType::Face).ok_or_else(|| regen_error("DRAFT_NO_NEUTRAL_PLANE"))?;
        model.shape(*face).face_plane().map_err(|_| regen_error("DRAFT_NO_NEUTRAL_PLANE"))?
    };
    let groups = by_body(model, field(definition, "draftFaces"), EntityType::Face)?;
    if groups.is_empty() {
        return Err(regen_error("DRAFT_SELECT_FACES"));
    }
    let mut results = Vec::new();
    for (b, faces) in groups {
        let op = occt::draft(&model.bodies[b].shape, &faces, pull, angle, neutral)
            .map_err(|_| regen_error("DRAFT_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

/// A draft of faces each about its own reference edges: each face turns about the plane through its first
/// reference edge square to the pull direction, at the draft's angle or its own. The faces and edges are given as
/// options, or, with `inferReferences` (std 3013 on), found from the parting edges as std's own
/// `getReferenceEntityDraftOptions` would (batch 049).
fn draft_about_entities(model: &mut Model, id: FeatureId, definition: &Value, pull: Vec3, angle: f64) -> R<Value> {
    let options: Vec<(EntityRef, Shape, f64)> = if values::opt_bool(definition, "inferReferences")?.unwrap_or(false) {
        inferred_draft_options(model, definition, pull, angle)?
    } else {
        let mut out = Vec::new();
        for option in
            field(definition, "referenceEntityDraftOptions").as_array().map(|a| a.to_vec()).unwrap_or_default()
        {
            let faces = query::evaluate(model, field(&option, "face"))?;
            let Some(face) = faces.into_iter().find(|r| r.kind == EntityType::Face) else { continue };
            let references = query::evaluate(model, field(&option, "references"))?;
            let edge =
                references.iter().find(|r| r.kind == EntityType::Edge).ok_or_else(|| regen_error("DRAFT_FAILED"))?;
            let own = match field(&option, "angle") {
                v if v.is_undefined() => angle,
                v => values::angle(v, "opDraft option angle")?,
            };
            out.push((face, model.shape(*edge).clone(), own));
        }
        out
    };
    let mut groups: Vec<(usize, Vec<Shape>, Vec<occt::Frame>, Vec<f64>)> = Vec::new();
    for (face, edge, own) in options {
        let at = edge.edge_tangent(0.5, false).map_err(|_| regen_error("DRAFT_FAILED"))?.origin;
        let neutral = occt::Frame { origin: at, normal: pull, x: values::canonical_x(pull) };
        let shape = model.shape(face).clone();
        match groups.iter_mut().find(|(b, _, _, _)| *b == face.body) {
            Some((_, shapes, frames, angles)) => {
                shapes.push(shape);
                frames.push(neutral);
                angles.push(own);
            }
            None => groups.push((face.body, vec![shape], vec![neutral], vec![own])),
        }
    }
    if groups.is_empty() {
        return Err(regen_error("DRAFT_SELECT_FACES"));
    }
    let mut results = Vec::new();
    for (b, faces, neutrals, angles) in groups {
        let op = occt::draft_angles(&model.bodies[b].shape, &faces, pull, &angles, &neutrals)
            .map_err(|_| regen_error("DRAFT_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

/// The faces a parting line draft turns, each with its parting edge and angle, from `inferReferenceOptions`: each
/// parting edge's two faces ordered by steepness (the face's normal along the edge's direction in it × the pull
/// direction), the more along the pull first; one side takes the first (or, not `alongPull`, the second) at the
/// draft's angle; symmetric and two sided ones take both, the face away from the pull at the draft's angle and the
/// one along it at its negative or at the second angle.
fn inferred_draft_options(
    model: &Model,
    definition: &Value,
    pull_vec: Vec3,
    angle: f64,
) -> R<Vec<(EntityRef, Shape, f64)>> {
    let options = field(definition, "inferReferenceOptions");
    let flag = |key: &str| options.get(key).and_then(Value::as_bool).unwrap_or(false);
    let pull_direction = !flag("negatePullVec");
    let second_pull_direction = !flag("negateSecondPullDirection");
    // std's raw pull direction: pullVec is it, or its negative with `pullDirection`.
    let raw = if pull_direction { values::scale(pull_vec, -1.0) } else { pull_vec };
    let sides = options.get("partingLineSides").and_then(Value::as_str).unwrap_or("ONE_SIDED").to_string();
    let along_pull = options.get("alongPull").and_then(Value::as_bool).unwrap_or(true);
    let away_angle = angle;
    let along_angle = match sides.as_str() {
        "SYMMETRIC" => -angle,
        "TWO_SIDED" => {
            let second = values::angle(field(options, "secondAngle"), "opDraft secondAngle")?;
            if pull_direction == second_pull_direction { second } else { -second }
        }
        _ => angle,
    };
    let edges: Vec<EntityRef> = query::evaluate(model, field(options, "partingEdges"))?
        .into_iter()
        .filter(|r| r.kind == EntityType::Edge)
        .collect();
    if edges.is_empty() {
        return Err(regen_error("DRAFT_SELECT_PARTING_EDGES"));
    }
    let mut out = Vec::new();
    for e in edges {
        let edge = model.shape(e).clone();
        let mid = edge.edge_tangent(0.5, false).map_err(|_| regen_error("DRAFT_FAILED"))?;
        let body = &model.bodies[e.body];
        let mut data: Vec<(EntityRef, f64)> = Vec::new();
        for (index, f) in body.faces.iter().enumerate() {
            if !f.shape.sub_shapes(ShapeKind::Edge).iter().any(|x| x.is_same(&edge)) {
                continue;
            }
            let (u, v) = f.shape.face_fraction(mid.origin).map_err(|_| regen_error("DRAFT_FAILED"))?;
            let n = f.shape.face_tangent_plane(u, v).map_err(|_| regen_error("DRAFT_FAILED"))?.normal;
            // The edge's direction in the face's loop: the face on its left, the normal up.
            let left = values::cross(n, mid.direction);
            let inside = occt::vertex(values::add(mid.origin, values::scale(left, 1e-6)))
                .and_then(|p| p.distance(&f.shape))
                .is_ok_and(|d| d.distance < 1e-7);
            let along = if inside { mid.direction } else { values::scale(mid.direction, -1.0) };
            if values::norm(values::cross(along, raw)) < 1e-9 {
                return Err(regen_error("DRAFT_PARALLEL_PARTING_EDGE"));
            }
            let across = values::normalize(values::cross(along, raw)).ok_or_else(|| regen_error("DRAFT_FAILED"))?;
            data.push((EntityRef { body: e.body, kind: EntityType::Face, index }, values::dot(n, across)));
        }
        if data.is_empty() {
            return Err(regen_error("DRAFT_FAILED"));
        }
        if data.len() == 1 {
            data.push(data[0]);
        }
        if data[1].1 > data[0].1 {
            data.swap(0, 1);
        }
        if sides == "ONE_SIDED" {
            let (face, _) = data[if along_pull { 0 } else { 1 }];
            out.push((face, edge.clone(), angle));
        } else {
            for (face, steepness) in &data {
                out.push((*face, edge.clone(), if *steepness > 0.0 { along_angle } else { away_angle }));
            }
        }
    }
    Ok(out)
}

/// A sheet whose faces are planar quadrilaterals each running between two planes along straight sides (a thin
/// loft's between its two profiles), carried past the planes: each face's corners moved out along its sides.
fn carried_past(sheet: &Shape, planes: &[occt::Frame], margin: f64) -> R<Shape> {
    use crate::values::{add, dot, norm, scale, sub};
    let failed = || {
        error("@opThicken: trimming to side surfaces of faces other than planar quadrilaterals between them isn't supported yet")
    };
    let [p0, p1] = planes else { return Err(failed()) };
    let on = |p: occt::Vec3, plane: &occt::Frame| dot(sub(p, plane.origin), plane.normal).abs() < 1e-7;
    let points = |s: &Shape| -> Vec<occt::Vec3> { s.sub_shapes(ShapeKind::Vertex).iter().filter_map(|v| v.vertex_point().ok()).collect() };
    let mut faces = Vec::new();
    for face in sheet.sub_shapes(ShapeKind::Face) {
        let plane = face.face_plane().map_err(|_| failed())?;
        let corners = points(&face);
        let low: Vec<occt::Vec3> = corners.iter().copied().filter(|c| on(*c, p0)).collect();
        let high: Vec<occt::Vec3> = corners.iter().copied().filter(|c| on(*c, p1)).collect();
        if corners.len() != 4 || low.len() != 2 || high.len() != 2 {
            return Err(failed());
        }
        // Each low corner's side runs to the high corner an edge joins it to.
        let joined = |a: occt::Vec3, b: occt::Vec3| {
            face.sub_shapes(ShapeKind::Edge).iter().any(|e| {
                let ends = points(e);
                ends.len() == 2
                    && ((norm(sub(ends[0], a)) < 1e-9 && norm(sub(ends[1], b)) < 1e-9)
                        || (norm(sub(ends[0], b)) < 1e-9 && norm(sub(ends[1], a)) < 1e-9))
            })
        };
        let mut sides = Vec::new();
        for &a in &low {
            let b = *high.iter().find(|&&b| joined(a, b)).ok_or_else(failed)?;
            let d = sub(b, a);
            let k = 4.0 * margin / norm(d).max(1e-12);
            sides.push((sub(a, scale(d, k)), add(b, scale(d, k))));
        }
        let [(a0, b0), (a1, b1)] = sides[..] else { return Err(failed()) };
        let edges = [occt::line_edge(a0, a1), occt::line_edge(a1, b1), occt::line_edge(b1, b0), occt::line_edge(b0, a0)]
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(crate::occt_error)?;
        let mut extended = occt::fill_edges(&edges).map_err(crate::occt_error)?;
        if dot(extended.face_plane().map_err(crate::occt_error)?.normal, plane.normal) < 0.0 {
            extended = extended.reversed();
        }
        faces.push(extended);
    }
    occt::sew_faces(&faces, 1e-7).map_err(crate::occt_error)
}

pub fn thicken(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let t1 = values::length(field(definition, "thickness1"), "opThicken thickness1")?;
    let t2 = values::length(field(definition, "thickness2"), "opThicken thickness2")?;
    let keep_tools = values::opt_bool(definition, "keepTools")?.unwrap_or(true);
    let found = query::evaluate(model, field(definition, "entities"))?;
    if found.is_empty() {
        return Err(regen_error("THICKEN_SELECT_ENTITIES"));
    }
    // Faces (of solids or sheets) and whole sheet bodies, each thickened on its own. What each makes is tracked back
    // to the entities of the body it came from.
    // Faces of one body are thickened together: all of a sheet's faces as the sheet itself.
    let mut inputs: Vec<(Shape, usize)> = Vec::new();
    let mut whole_sheets = Vec::new();
    let mut faces_by_body: Vec<(usize, Vec<Shape>)> = Vec::new();
    for r in &found {
        match r.kind {
            EntityType::Face => match faces_by_body.iter_mut().find(|(b, _)| *b == r.body) {
                Some((_, faces)) => faces.push(model.shape(*r).clone()),
                None => faces_by_body.push((r.body, vec![model.shape(*r).clone()])),
            },
            EntityType::Body if model.body(*r).body_type == BodyType::Sheet => {
                inputs.push((model.body(*r).shape.clone(), r.body));
                whole_sheets.push(r.body);
            }
            _ => {}
        }
    }
    for (b, faces) in faces_by_body {
        if whole_sheets.contains(&b) {
            continue;
        }
        let body = &model.bodies[b];
        let input = if body.body_type == BodyType::Sheet && faces.len() == body.faces.len() {
            body.shape.clone()
        } else if faces.len() == 1 {
            faces[0].clone()
        } else {
            occt::compound(&faces)
        };
        inputs.push((input, b));
    }
    let info = Info::new(id.clone());
    let start_info = Rc::new(Info {
        created_by: id.clone(),
        sketch_entity: None,
        cap: Some(Cap::Start),
        dependency: None,
        ..Default::default()
    });
    let end_info = Rc::new(Info {
        created_by: id,
        sketch_entity: None,
        cap: Some(Cap::End),
        dependency: None,
        ..Default::default()
    });
    let (mut start_cap, mut end_cap) = (ShapeSet::default(), ShapeSet::default());
    let mut made = Vec::new();
    // Side surfaces supplied (a thin loft's ends trimmed to its profiles' planes, batch 076 c008): each sheet carried
    // past them along its straight sides, thickened, and cut back to them.
    let side_planes: Vec<occt::Frame> = match field(definition, "sideGeometryRule") {
        rule if field(rule, "type").as_str() == Some("SUPPLIED") => {
            let mut planes = Vec::new();
            for group in field(rule, "sideSurfacesEdgeFaceGroups").as_array().map(|a| a.to_vec()).unwrap_or_default() {
                let face = query::evaluate(model, field(&group, "face"))?
                    .into_iter()
                    .find(|r| r.kind == EntityType::Face)
                    .ok_or_else(|| regen_error("THICKEN_FAILED"))?;
                planes.push(
                    model
                        .shape(face)
                        .face_plane()
                        .map_err(|_| error("@opThicken: side surfaces other than planes aren't supported yet"))?,
                );
            }
            planes
        }
        _ => Vec::new(),
    };
    if !side_planes.is_empty() {
        for (input, _) in &inputs {
            let inside = input.sub_shapes(ShapeKind::Face).first().and_then(|f| f.face_inner_point().ok());
            let Some(inside) = inside else { return Err(regen_error("THICKEN_FAILED")) };
            let extended = carried_past(input, &side_planes, t1.abs() + t2.abs())?;
            let mut solid = occt::thicken(&extended, t1, t2).map_err(|_| regen_error("THICKEN_FAILED"))?.shape();
            for plane in &side_planes {
                // Away from the sheet, past the plane.
                let n = if values::dot(values::sub(inside, plane.origin), plane.normal) >= 0.0 {
                    values::scale(plane.normal, -1.0)
                } else {
                    plane.normal
                };
                let bounds = solid.bounding_box(false).map_err(crate::occt_error)?;
                let size = values::norm(values::sub(bounds.max, bounds.min)) * 4.0 + 1.0;
                let frame = occt::Frame { origin: plane.origin, normal: n, x: values::canonical_x(n) };
                let slab = occt::plane_face(frame, -size, size, -size, size).map_err(crate::occt_error)?;
                let away = occt::prism(&slab, values::scale(n, size)).map_err(crate::occt_error)?.shape();
                solid = occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(&solid), &[away], true)
                    .map_err(|_| regen_error("THICKEN_FAILED"))?
                    .shape();
            }
            for piece in solid.sub_shapes(ShapeKind::Solid) {
                made.push((piece, Rc::new(Origins::default())));
            }
        }
        if !keep_tools {
            model.remove_bodies(&whole_sheets);
        }
        for (solid, origins) in made {
            model.add_body(NewBody::new(solid, BodyType::Solid, info.clone()), &origins, &info, &|_| None);
        }
        return Ok(Value::UNDEFINED);
    }
    for (input, body) in &inputs {
        let op = occt::thicken(input, t1, t2).map_err(|_| regen_error("THICKEN_FAILED"))?;
        let entities: Vec<&Entity> = model.bodies[*body].all_entities().collect();
        let origins = Rc::new(Origins::derived(&op, &entities));
        for cap in op.first() {
            start_cap.insert_all(&cap);
        }
        for cap in op.last() {
            end_cap.insert_all(&cap);
        }
        for solid in op.shape().sub_shapes(ShapeKind::Solid) {
            made.push((solid, origins.clone()));
        }
    }
    if !keep_tools {
        model.remove_bodies(&whole_sheets);
    }
    let special = |s: &Shape| {
        if end_cap.contains(s) {
            Some(end_info.clone())
        } else if start_cap.contains(s) {
            Some(start_info.clone())
        } else {
            None
        }
    };
    for (solid, origins) in made {
        model.add_body(NewBody::new(solid, BodyType::Solid, info.clone()), &origins, &info, &special);
    }
    Ok(Value::UNDEFINED)
}

/// A whole cylindrical face offset by a tube added or taken away between its radius and the new one, along the
/// length its circular edges bound: OCCT's offset carries on into neighbours on the same cylinder (an external
/// thread's undercut, batch 031). `None` for anything but a single whole cylinder.
fn cylinder_by_tube(body: &occt::Shape, faces: &[occt::Shape], distance: f64) -> R<Option<occt::Operation>> {
    let [face] = faces else { return Ok(None) };
    let Ok(s) = face.surface_definition() else { return Ok(None) };
    if s.kind != 1 {
        return Ok(None);
    }
    let (o, a, u, r) = (s.frame.origin, s.frame.normal, s.frame.x, s.radius1);
    // Its ends: two whole circles round the axis.
    let mut heights = Vec::new();
    for edge in face.visible_sub_shapes(occt::ShapeKind::Edge) {
        if edge.curve_type().ok() != Some(occt::CurveType::Circle) {
            return Ok(None);
        }
        let Ok(c) = edge.edge_tangent(0.0, false) else { return Ok(None) };
        heights.push(values::dot(values::sub(c.origin, o), a));
    }
    let [h0, h1] = heights[..] else { return Ok(None) };
    let p = face.face_inner_point().map_err(occt_error)?;
    let (fu, fv) = face.face_fraction(p).map_err(occt_error)?;
    let n = face.face_tangent_plane(fu, fv).map_err(occt_error)?.normal;
    let rel = values::sub(p, o);
    let outward = values::dot(n, values::sub(rel, values::scale(a, values::dot(rel, a)))) > 0.0;
    let new_radius = r + if outward { distance } else { -distance };
    if new_radius <= 1e-9 {
        return Err(regen_error("DIRECT_EDIT_OFFSET_FACE_FAILED"));
    }
    let (r0, r1) = (r.min(new_radius), r.max(new_radius));
    let at = |radius: f64, h: f64| values::add(o, values::add(values::scale(u, radius), values::scale(a, h)));
    let corners = [at(r0, h0), at(r1, h0), at(r1, h1), at(r0, h1)];
    let edges = (0..4)
        .map(|i| occt::line_edge(corners[i], corners[(i + 1) % 4]))
        .collect::<Result<Vec<_>, _>>()
        .map_err(occt_error)?;
    let section = occt::fill_edges(&edges).map_err(occt_error)?;
    let tube = occt::revolve(&section, occt::Axis { origin: o, direction: a }, std::f64::consts::TAU)
        .map_err(occt_error)?
        .shape();
    // Taking material away where the face moves into the body, adding it where it moves out.
    let kind = if (new_radius < r) == outward { occt::BooleanKind::Subtraction } else { occt::BooleanKind::Union };
    let op = occt::boolean(kind, std::slice::from_ref(body), &[tube], true)
        .map_err(|_| regen_error("DIRECT_EDIT_OFFSET_FACE_FAILED"))?;
    Ok(Some(op))
}

pub fn offset_face(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let distance = values::length(field(definition, "offsetDistance"), "opOffsetFace offsetDistance")?;
    let groups = by_body(model, field(definition, "moveFaces"), EntityType::Face)?;
    if groups.is_empty() {
        return Err(regen_error("DIRECT_EDIT_OFFSET_FACE_SELECT"));
    }
    let mut results = Vec::new();
    for (b, faces) in groups {
        let op = match cylinder_by_tube(&model.bodies[b].shape, &faces, distance)? {
            Some(op) => op,
            None => occt::offset_faces(&model.bodies[b].shape, &faces, distance)
                .map_err(|_| regen_error("DIRECT_EDIT_OFFSET_FACE_FAILED"))?,
        };
        results.push((b, op));
    }
    // The faces and edges the offset rebuilt without moving keep their identities, as in Onshape.
    let info = crate::model::Info::new(id.clone());
    for (b, op) in results.into_iter().rev() {
        let old = model.bodies[b].clone();
        let origins = crate::model::Origins::of_operation_by_geometry(&op, &[&old]);
        model.remove_bodies(&[b]);
        model.add_body(crate::model::NewBody::replacing(&old, op.shape()), &origins, &info, &|_| None);
    }
    Ok(Value::UNDEFINED)
}

pub fn delete_face(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let leave_open = values::opt_bool(definition, "leaveOpen")?.unwrap_or(false);
    let mut groups = by_body(model, field(definition, "deleteFaces"), EntityType::Face)?;
    if groups.is_empty() {
        return Err(regen_error("DIRECT_EDIT_DELETE_SELECT_FACES"));
    }
    // Faces of sheets just go: a sheet with none left goes too, the rest keep their identities.
    let (sheets, solids): (Vec<_>, Vec<_>) =
        groups.drain(..).partition(|(b, _)| model.bodies[*b].body_type == BodyType::Sheet);
    groups = solids;
    let mut emptied = Vec::new();
    let mut trimmed = Vec::new();
    for (b, faces) in sheets {
        let old = model.bodies[b].clone();
        if old.faces.iter().all(|f| faces.iter().any(|g| g.is_same(&f.shape))) {
            emptied.push(b);
            continue;
        }
        let sheet = occt::remove_faces_open(&old.shape, &faces).map_err(occt_error)?;
        // Edges that lost a face beside them are changed (new ids, tracked back); the rest keep their identities.
        let lost_neighbour =
            |e: &Shape| faces.iter().any(|f| f.sub_shapes(ShapeKind::Edge).iter().any(|x| x.is_same(e)));
        let mut origins = Origins::default();
        for entity in old.all_entities() {
            if entity.shape.kind() == Some(ShapeKind::Edge) && lost_neighbour(&entity.shape) {
                origins.inherit(&entity.shape, entity);
            } else {
                origins.keep(entity);
            }
        }
        let origins = std::rc::Rc::new(origins);
        // A sheet that falls apart becomes a body for each piece; the first keeps the body's identity.
        let pieces = occt::connected_shells(&sheet).map_err(occt_error)?;
        let pieces = if pieces.is_empty() { vec![sheet] } else { pieces };
        for (k, piece) in pieces.into_iter().enumerate() {
            let tid = (k == 0).then_some(old.tid);
            trimmed.push((
                b,
                NewBody { body_type: BodyType::Sheet, tid, ..NewBody::replacing(&old, piece) },
                origins.clone(),
            ));
        }
    }
    if !emptied.is_empty() || !trimmed.is_empty() {
        let info = Info::new(id.clone());
        let mut removed = emptied;
        removed.extend(trimmed.iter().map(|(b, _, _)| *b));
        // Indices of the solids still to do shift as bodies go; find them again by transient id.
        let solid_tids: Vec<(crate::model::Tid, Vec<Shape>)> =
            groups.drain(..).map(|(b, faces)| (model.bodies[b].tid, faces)).collect();
        model.remove_bodies(&removed);
        for (_, new, origins) in trimmed {
            model.add_body(new, origins.as_ref(), &info, &|_| None);
        }
        for (tid, faces) in solid_tids {
            if let Some(b) = model.bodies.iter().position(|body| body.tid == tid) {
                groups.push((b, faces));
            }
        }
        if groups.is_empty() {
            return Ok(Value::UNDEFINED);
        }
    }
    if leave_open {
        // The body becomes a sheet of its other faces; they keep their identities.
        let info = Info::new(id);
        let mut added = Vec::new();
        for (b, faces) in &groups {
            let old = model.bodies[*b].clone();
            let sheet = occt::remove_faces_open(&old.shape, faces).map_err(occt_error)?;
            let mut origins = Origins::default();
            for entity in old.all_entities() {
                origins.keep(entity);
            }
            added.push((NewBody { body_type: BodyType::Sheet, ..NewBody::replacing(&old, sheet) }, origins));
        }
        let removed: Vec<usize> = groups.iter().map(|(b, _)| *b).collect();
        model.remove_bodies(&removed);
        for (new, origins) in added {
            model.add_body(new, &origins, &info, &|_| None);
        }
        return Ok(Value::UNDEFINED);
    }
    // Capping replaces a face with a planar one, which only works when the face's boundary is flat; Onshape leaves
    // other faces (a hole through a block, say) as they are. (Batch 003, c039.)
    let cap = values::opt_bool(definition, "capVoid")?.unwrap_or(false);
    let mut results = Vec::new();
    for (b, faces) in groups {
        let faces: Vec<Shape> = if cap { faces.into_iter().filter(boundary_is_flat).collect() } else { faces };
        if faces.is_empty() {
            continue;
        }
        let op = occt::delete_faces(&model.bodies[b].shape, &faces)
            .map_err(|_| regen_error("DIRECT_EDIT_DELETE_FACE_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

/// Whether a face's boundary lies in one plane.
fn boundary_is_flat(face: &Shape) -> bool {
    let points: Vec<occt::Vec3> =
        face.sub_shapes(ShapeKind::Vertex).iter().filter_map(|v| v.vertex_point().ok()).collect();
    let edges = face.sub_shapes(ShapeKind::Edge);
    let mut samples = points;
    for edge in &edges {
        for t in [0.25, 0.5, 0.75] {
            if let Ok(axis) = edge.edge_tangent(t, false) {
                samples.push(axis.origin);
            }
        }
    }
    if samples.len() < 3 {
        return true;
    }
    let origin = samples[0];
    let mut normal = None;
    for i in 1..samples.len() {
        for j in i + 1..samples.len() {
            let n = values::cross(values::sub(samples[i], origin), values::sub(samples[j], origin));
            if values::norm(n) > 1e-12 {
                normal = values::normalize(n);
                break;
            }
        }
        if normal.is_some() {
            break;
        }
    }
    let Some(normal) = normal else { return true };
    samples.iter().all(|p| values::dot(values::sub(*p, origin), normal).abs() < 1e-7)
}

/// The plane of the first planar face a query finds.
/// The plane of a query's first face, if that face is planar: the template is the first face, whatever follows (a
/// solid cylinder's faces as a template are its side, batch 118 c003).
fn plane_of(model: &Model, query: &Value) -> R<Option<occt::Frame>> {
    let found = query::evaluate(model, query)?;
    Ok(found.iter().find(|r| r.kind == EntityType::Face).and_then(|r| model.shape(*r).face_plane().ok()))
}

pub fn replace_face(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let offset = match field(definition, "offset") {
        v if v.is_undefined() => 0.0,
        v => values::length(v, "opReplaceFace offset")?,
    };
    let Some(mut target) = plane_of(model, field(definition, "templateFace"))? else {
        return replace_with_surface(model, id, definition, offset);
    };
    target.origin = values::add(target.origin, values::scale(target.normal, offset));
    let groups = by_body(model, field(definition, "replaceFaces"), EntityType::Face)?;
    if groups.is_empty() {
        return Err(regen_error("DIRECT_EDIT_REPLACE_FACE_SELECT"));
    }
    let mut results = Vec::new();
    // The faces replaced, by body: the face on the target plane takes over what they were, and its edges run as the
    // old face's did.
    let mut replaced: Vec<(crate::model::Tid, Entity, Vec<EdgeRun>)> = Vec::new();
    for (b, faces) in groups {
        let body = &model.bodies[b];
        if let Some(first) = body.faces.iter().find(|f| faces.iter().any(|g| g.is_same(&f.shape))) {
            replaced.push((body.tid, first.clone(), edge_runs(model, b, &first.shape)));
        }
        let mut shape = body.shape.clone();
        let mut last = None;
        for face in &faces {
            // Up to the plane of a face next to it, the face is taken away, that face healing over it (batch 197 c004:
            // a cylinder's rounded top edge up to the plane of its top).
            let op = match occt::replace_planar_face(&shape, face, target) {
                Ok(op) => op,
                Err(_) if neighbour_on_plane(&shape, face, target) => occt::delete_faces(&shape, std::slice::from_ref(face))
                    .map_err(|_| regen_error("DIRECT_EDIT_REPLACE_FACE_FAILED"))?,
                Err(_) => return Err(regen_error("DIRECT_EDIT_REPLACE_FACE_FAILED")),
            };
            shape = op.shape();
            last = Some(op);
        }
        if let Some(op) = last {
            results.push((b, op));
        }
    }
    replace_bodies(model, &id, results);
    // The new face keeps the replaced face's info, attributes and identity, as Onshape's does (frame's mitres rely on
    // the cap faces' attributes surviving).
    for (tid, old, runs) in replaced {
        let Some(b) = model.bodies.iter().position(|body| body.tid == tid) else { continue };
        let on_target = model.bodies[b].faces.iter().position(|f| {
            f.shape.face_plane().is_ok_and(|p| {
                values::norm(values::cross(p.normal, target.normal)) < 1e-9
                    && values::dot(values::sub(p.origin, target.origin), target.normal).abs() < 1e-8
            })
        });
        let Some(index) = on_target else { continue };
        model.update_body(b, |body| {
            let face = &mut body.faces[index];
            face.info = old.info.clone();
            face.attributes = old.attributes.clone();
            let mut identity: Vec<crate::model::Tid> = face.ancestry.identity.to_vec();
            identity.push(old.tid);
            identity.extend(old.ancestry.identity.iter().copied());
            face.ancestry.identity = identity.into();
        });
        let face = model.bodies[b].faces[index].shape.clone();
        keep_edge_runs(model, b, &face, &runs);
    }
    Ok(Value::UNDEFINED)
}

/// Whether a face next to `face` in `solid` lies on the plane `target`.
fn neighbour_on_plane(solid: &Shape, face: &Shape, target: occt::Frame) -> bool {
    face.sub_shapes(ShapeKind::Edge).iter().any(|edge| {
        solid.ancestors(edge, ShapeKind::Face).into_iter().filter(|g| !g.is_same(face)).any(|g| {
            g.face_plane().is_ok_and(|p| {
                values::norm(values::cross(p.normal, target.normal)) < 1e-9
                    && values::dot(values::sub(p.origin, target.origin), target.normal).abs() < 1e-9
            })
        })
    })
}

/// A round (a convex cylindrical face tangent to two planar faces) replaced by a larger coaxial cylinder (batch 197
/// c005): the corner made sharp again, then cut back to the larger cylinder within the wedge between the round's
/// axis and the corner, its neighbours meeting the cylinder where it crosses them.
fn round_to_coaxial(solid: &Shape, face: &Shape, template: &Shape) -> R<Option<occt::Operation>> {
    use crate::values::{add, cross, dot, norm, scale, sub};
    let (Ok(a), Ok(b)) = (face.surface_definition(), template.surface_definition()) else { return Ok(None) };
    if a.kind != 1 || b.kind != 1 || b.radius1 <= a.radius1 + 1e-9 {
        return Ok(None);
    }
    let d = a.frame.normal;
    let o = a.frame.origin;
    if norm(cross(d, b.frame.normal)) > 1e-9 || norm(cross(sub(b.frame.origin, o), d)) > 1e-9 {
        return Ok(None);
    }
    let r = a.radius1;
    // Its two planar neighbours, each its radius from the axis and parallel to it, their normals out of the solid.
    let mut planes: Vec<occt::Frame> = Vec::new();
    for edge in face.sub_shapes(ShapeKind::Edge) {
        for g in solid.ancestors(&edge, ShapeKind::Face).into_iter().filter(|g| !g.is_same(face)) {
            if let Ok(p) = g.face_plane()
                && dot(p.normal, d).abs() < 1e-9
                && (dot(sub(p.origin, o), p.normal) - r).abs() < 1e-7
                && !planes.iter().any(|q| norm(sub(q.normal, p.normal)) < 1e-9)
            {
                planes.push(p);
            }
        }
    }
    let [p1, p2] = planes[..] else { return Ok(None) };
    let c = 1.0 + dot(p1.normal, p2.normal);
    if c < 1e-6 {
        return Ok(None);
    }
    // Along the axis, past the solid either way.
    let corners = solid.bounding_box(false).map_err(occt_error)?;
    let reach = norm(sub(corners.max, corners.min)) + 1.0;
    let base = sub(o, scale(d, reach));
    let (t1, t2) = (add(base, scale(p1.normal, r)), add(base, scale(p2.normal, r)));
    let corner = add(base, scale(add(p1.normal, p2.normal), r / c));
    let ring = [base, t1, corner, t2];
    let edges = (0..4)
        .map(|i| occt::line_edge(ring[i], ring[(i + 1) % 4]))
        .collect::<Result<Vec<_>, _>>()
        .map_err(occt_error)?;
    let wedge = occt::prism(&occt::fill_edges(&edges).map_err(occt_error)?, scale(d, 2.0 * reach)).map_err(occt_error)?.shape();
    let x = p1.normal;
    let circle = occt::circle_edge(occt::Frame { origin: base, normal: d, x }, b.radius1).map_err(occt_error)?;
    let cylinder = occt::prism(&occt::fill_edges(&[circle]).map_err(occt_error)?, scale(d, 2.0 * reach)).map_err(occt_error)?.shape();
    let healed = occt::delete_faces(solid, std::slice::from_ref(face)).map_err(|_| regen_error("DIRECT_EDIT_REPLACE_FACE_FAILED"))?.shape();
    let outside = occt::boolean(occt::BooleanKind::Subtraction, &[wedge], &[cylinder], false).map_err(occt_error)?.shape();
    let op = occt::boolean(occt::BooleanKind::Subtraction, &[healed], &[outside], true)
        .map_err(|_| regen_error("DIRECT_EDIT_REPLACE_FACE_FAILED"))?;
    Ok(Some(op))
}

/// How an edge of a face ran: the face next to it across the edge (its transient id), and whether Onshape's edge
/// ran as the face's loop runs it.
struct EdgeRun {
    neighbour: crate::model::Tid,
    with_loop: bool,
}

/// The neighbour of a face of body `b` across an edge, as an entity.
fn neighbour_across<'a>(model: &'a Model, b: usize, face: &Shape, edge: &Shape) -> Option<&'a Entity> {
    let body = &model.bodies[b];
    let other = body.shape.ancestors(edge, ShapeKind::Face).into_iter().find(|g| !g.is_same(face))?;
    body.faces.iter().find(|f| f.shape.is_same(&other))
}

/// How each edge of a face of body `b` runs ([`EdgeRun`]).
fn edge_runs(model: &Model, b: usize, face: &Shape) -> Vec<EdgeRun> {
    let body = &model.bodies[b];
    let mut runs = Vec::new();
    for used in face.sub_shapes(ShapeKind::Edge) {
        let Some(k) = body.edges.iter().position(|e| e.shape.is_same(&used)) else { continue };
        let Some(neighbour) = neighbour_across(model, b, face, &used) else { continue };
        let onshape = super::onshape_edge(model, EntityRef { body: b, kind: EntityType::Edge, index: k });
        let Ok(a) = used.edge_tangent(0.5, false) else { continue };
        let Ok(o) = onshape.edge_fraction(a.origin, false).and_then(|t| onshape.edge_tangent(t, false)) else { continue };
        runs.push(EdgeRun { neighbour: neighbour.tid, with_loop: values::dot(a.direction, o.direction) > 0.0 });
    }
    runs
}

/// Onshape replaces a face's surface keeping its edges, each running as it did, with the faces round it recut: so
/// each edge of the new face runs as the old face's edge to the same neighbour did, along or against the face's loop
/// (batch 021 c009: a frame's mitred corner, whose inner edge a gusset is offset along).
fn keep_edge_runs(model: &mut Model, b: usize, face: &Shape, runs: &[EdgeRun]) {
    let mut senses = Vec::new();
    for used in face.sub_shapes(ShapeKind::Edge) {
        let Some(neighbour) = neighbour_across(model, b, face, &used) else { continue };
        let Some(run) =
            runs.iter().find(|r| neighbour.tid == r.neighbour || neighbour.ancestry.identity.contains(&r.neighbour))
        else {
            continue;
        };
        let Ok(a) = used.edge_tangent(0.5, false) else { continue };
        let d = if run.with_loop { a.direction } else { values::scale(a.direction, -1.0) };
        senses.push((used, (a.origin, d)));
    }
    model.update_body(b, |body| {
        for (used, sense) in &senses {
            if let Some(e) = body.edges.iter_mut().find(|e| e.shape.is_same(used)) {
                e.info = Rc::new(Info { sense: Some(*sense), ..(*e.info).clone() });
            }
        }
    });
}

/// How far a cylindrical or spherical face moves along its normal to lie on a template coaxial or concentric with it,
/// or `None` if the template isn't.
fn parallel_offset(face: &occt::Shape, template: &occt::Shape) -> R<Option<f64>> {
    let (Ok(a), Ok(b)) = (face.surface_definition(), template.surface_definition()) else { return Ok(None) };
    let on_axis = |p: occt::Vec3| {
        let d = values::sub(p, b.frame.origin);
        values::norm(values::cross(d, b.frame.normal)) < 1e-9
    };
    let same = match (a.kind, b.kind) {
        (1, 1) => values::norm(values::cross(a.frame.normal, b.frame.normal)) < 1e-9 && on_axis(a.frame.origin),
        (3, 3) => values::norm(values::sub(a.frame.origin, b.frame.origin)) < 1e-9,
        _ => false,
    };
    if !same {
        return Ok(None);
    }
    // Whether the face's normal points away from its axis or centre.
    let p = face.face_inner_point().map_err(occt_error)?;
    let (u, v) = face.face_fraction(p).map_err(occt_error)?;
    let normal = face.face_tangent_plane(u, v).map_err(occt_error)?.normal;
    let from = values::sub(p, a.frame.origin);
    let radial = if a.kind == 1 {
        values::sub(from, values::scale(a.frame.normal, values::dot(from, a.frame.normal)))
    } else {
        from
    };
    let outward = values::dot(normal, radial) > 0.0;
    Ok(Some((b.radius1 - a.radius1) * if outward { 1.0 } else { -1.0 }))
}

/// Planar faces replaced by a curved template face's surface (batch 020): see `occt::replace_face_with_surface`.
/// A solid of a cylindrical side and two planar caps square to its axis, its side replaced by a cylinder parallel to
/// it but off its axis, or of a conical side replaced by a cylinder along its axis: the template's cylinder (its
/// radius plus `offset`) between the caps. `None` otherwise.
fn off_axis_cylinder(
    solid: &occt::Shape,
    face: &occt::Shape,
    template: &occt::Shape,
    offset: f64,
) -> R<Option<occt::Operation>> {
    use values::{add, cross, dot, norm, scale, sub};
    let (Ok(a), Ok(b)) = (face.surface_definition(), template.surface_definition()) else { return Ok(None) };
    if !matches!(a.kind, 1 | 2) || b.kind != 1 || norm(cross(a.frame.normal, b.frame.normal)) > 1e-9 {
        return Ok(None);
    }
    let axis = b.frame.normal;
    // A coaxial cylinder moves the side out or in instead (a cone's goes over to the cylinder, batch 085 c001).
    if a.kind == 1 && norm(cross(sub(a.frame.origin, b.frame.origin), axis)) < 1e-9 {
        return Ok(None);
    }
    let faces = solid.sub_shapes(occt::ShapeKind::Face);
    if faces.len() != 3 {
        return Ok(None);
    }
    let mut heights = Vec::new();
    for f in faces.iter().filter(|f| !f.is_same(face)) {
        let Ok(plane) = f.face_plane() else { return Ok(None) };
        if norm(cross(plane.normal, axis)) > 1e-9 {
            return Ok(None);
        }
        heights.push(dot(sub(plane.origin, b.frame.origin), axis));
    }
    let [h0, h1] = heights[..] else { return Ok(None) };
    let (lo, hi) = (h0.min(h1), h0.max(h1));
    let radius = b.radius1 + offset;
    if radius <= 0.0 || hi - lo < 1e-9 {
        return Ok(None);
    }
    let frame = occt::Frame { origin: add(b.frame.origin, scale(axis, lo)), normal: axis, x: b.frame.x };
    let circle = occt::circle_edge(frame, radius).map_err(occt_error)?;
    let disc = occt::fill_edges(&[circle]).map_err(occt_error)?;
    Ok(Some(occt::prism(&disc, scale(axis, hi - lo)).map_err(occt_error)?))
}

/// A solid of a cylindrical or conical side and two planar caps square to its axis, its side replaced by a sphere
/// crossing both caps' planes: the ball between the caps (batch 118 c004: a barrel). Only with the face's material on
/// the sphere's inside (`oppositeSense` false, the template a sphere's outward face).
fn capped_side_to_sphere(
    solid: &occt::Shape,
    face: &occt::Shape,
    template: &occt::Shape,
    offset: f64,
    opposite: bool,
) -> R<Option<occt::Operation>> {
    use values::{add, cross, dot, norm, scale, sub};
    let (Ok(a), Ok(b)) = (face.surface_definition(), template.surface_definition()) else { return Ok(None) };
    if !matches!(a.kind, 1 | 2) || b.kind != 3 || opposite {
        return Ok(None);
    }
    // The template's face looks outward from its centre.
    let outward = template.face_inner_point().ok().and_then(|p| {
        let (u, v) = template.face_fraction(p).ok()?;
        Some(dot(template.face_tangent_plane(u, v).ok()?.normal, sub(p, b.frame.origin)) > 0.0)
    });
    if outward != Some(true) {
        return Ok(None);
    }
    let axis = a.frame.normal;
    let faces = solid.sub_shapes(occt::ShapeKind::Face);
    if faces.len() != 3 {
        return Ok(None);
    }
    let mut heights = Vec::new();
    for f in faces.iter().filter(|f| !f.is_same(face)) {
        let Ok(plane) = f.face_plane() else { return Ok(None) };
        if norm(cross(plane.normal, axis)) > 1e-9 {
            return Ok(None);
        }
        heights.push(dot(sub(plane.origin, a.frame.origin), axis));
    }
    let [h0, h1] = heights[..] else { return Ok(None) };
    let (lo, hi) = (h0.min(h1), h0.max(h1));
    let radius = b.radius1 + offset;
    let centre = b.frame.origin;
    let along = dot(sub(centre, a.frame.origin), axis);
    // The ball must reach past both caps' planes.
    if radius <= 0.0 || (along - lo).abs() >= radius || (hi - along).abs() >= radius {
        return Ok(None);
    }
    // The slab between the caps: a disc wider than the ball, extruded from one cap's plane to the other's.
    let size = 2.0 * (radius + norm(sub(centre, a.frame.origin)));
    let frame = occt::Frame { origin: add(a.frame.origin, scale(axis, lo)), normal: axis, x: a.frame.x };
    let disc = occt::fill_edges(&[occt::circle_edge(frame, size).map_err(occt_error)?]).map_err(occt_error)?;
    let slab = occt::prism(&disc, scale(axis, hi - lo)).map_err(occt_error)?.shape();
    let ball = occt::sphere(centre, radius).map_err(occt_error)?;
    let op = occt::boolean(occt::BooleanKind::Intersection, &[slab], &[ball], true).map_err(occt_error)?;
    Ok(Some(op))
}

fn replace_with_surface(model: &mut Model, id: FeatureId, definition: &Value, offset: f64) -> R<Value> {
    let opposite = values::opt_bool(definition, "oppositeSense")?.unwrap_or(false);
    let template_ref = query::evaluate(model, field(definition, "templateFace"))?
        .into_iter()
        .find(|r| r.kind == EntityType::Face)
        .ok_or_else(|| regen_error("DIRECT_EDIT_REPLACE_FACE_SELECT_TEMPLATE"))?;
    let template = model.shape(template_ref).clone();
    let groups = by_body(model, field(definition, "replaceFaces"), EntityType::Face)?;
    if groups.is_empty() {
        return Err(regen_error("DIRECT_EDIT_REPLACE_FACE_SELECT"));
    }
    // Onshape won't replace a face up to a sheet lofted between arcs, a rational B-spline in Onshape (batch 067 c005;
    // batches 108, 111: between equal arcs, either way round or wider, and extended first, too), though it will up
    // to a loft between lines or the same arc extruded. The engine's lofts between arcs aren't rational (and between
    // equal arcs are cylinders): the loft marks them (`Info::lofted`).
    if model.info(template_ref).lofted {
        return Err(regen_error("DIRECT_EDIT_REPLACE_FACE_FAILED"));
    }
    // The new face takes the template's sense (or its opposite, `oppositeSense`): one facing against the replaced
    // face would turn the body inside out there. Onshape refuses that up to a parallel surface (batch 189 c002: a
    // hole up to a smaller coaxial cylinder, the cylinder's face outward from its axis, the hole's towards it), and
    // otherwise takes the face away, its neighbours healing over it (batch 197 c001: a round up to a cone across it
    // facing in).
    let mut against: Vec<Shape> = Vec::new();
    let samples: Vec<occt::Frame> = (0..9)
        .flat_map(|i| (0..9).map(move |j| ((i as f64 + 0.5) / 9.0, (j as f64 + 0.5) / 9.0)))
        .filter_map(|(fu, fv)| template.face_tangent_plane(fu, fv).ok())
        .collect();
    for (_, faces) in &groups {
        for face in faces {
            let Ok(at) = face.face_tangent_plane(0.5, 0.5) else { continue };
            let nearest = samples.iter().min_by(|x, y| {
                values::norm(values::sub(x.origin, at.origin))
                    .total_cmp(&values::norm(values::sub(y.origin, at.origin)))
            });
            if let Some(t) = nearest {
                let sense = values::dot(at.normal, t.normal) * if opposite { -1.0 } else { 1.0 };
                if sense < -1e-9 {
                    if parallel_offset(face, &template)?.is_some() {
                        return Err(regen_error("DIRECT_EDIT_REPLACE_FACE_FAILED"));
                    }
                    against.push(face.clone());
                }
            }
        }
    }
    let mut results = Vec::new();
    for (b, faces) in groups {
        let mut shape = model.bodies[b].shape.clone();
        let mut last = None;
        for face in &faces {
            if against.iter().any(|f| f.is_same(face)) {
                let op = occt::delete_faces(&shape, std::slice::from_ref(face))
                    .map_err(|_| regen_error("DIRECT_EDIT_REPLACE_FACE_FAILED"))?;
                // Where the neighbours can't heal over it, refused (batch 087 c005: a cone frustum's side up to a
                // steeper coaxial cone, its caps parallel).
                let count = |s: &Shape| s.sub_shapes(ShapeKind::Face).len();
                if count(&op.shape()) >= count(&shape) {
                    return Err(regen_error("DIRECT_EDIT_REPLACE_FACE_FAILED"));
                }
                shape = op.shape();
                last = Some(op);
                continue;
            }
            if let Some(op) = round_to_coaxial(&shape, face, &template)? {
                shape = op.shape();
                last = Some(op);
                continue;
            }
            // A plain cylinder's side replaced by a parallel cylinder off its axis: that cylinder between the
            // caps (batch 058 c002).
            if let Some(op) = off_axis_cylinder(&shape, face, &template, offset)? {
                shape = op.shape();
                last = Some(op);
                continue;
            }
            if let Some(op) = capped_side_to_sphere(&shape, face, &template, offset, opposite)? {
                shape = op.shape();
                last = Some(op);
                continue;
            }
            // A cylinder replaced by a coaxial one (or a sphere by a concentric one) moves out or in by the
            // difference of their radii (batch 028).
            let op = match parallel_offset(face, &template)? {
                Some(distance) => occt::offset_faces(&shape, std::slice::from_ref(face), distance + offset),
                None => occt::replace_face_with_surface(&shape, face, &template, offset),
            }
            .map_err(|_| regen_error("DIRECT_EDIT_REPLACE_FACE_FAILED"))?;
            shape = op.shape();
            last = Some(op);
        }
        if let Some(op) = last {
            results.push((b, op));
        }
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

pub fn move_face(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let t = super::transform(field(definition, "transform"))?;
    let groups = by_body(model, field(definition, "moveFaces"), EntityType::Face)?;
    if groups.is_empty() {
        return Err(regen_error("DIRECT_EDIT_MOVE_FACE_SELECT"));
    }
    let linear_is_identity =
        (0..3).all(|i| (0..3).all(|j| (t.m[i * 4 + j] - f64::from(u8::from(i == j))).abs() < 1e-12));
    let translation = occt::Vec3::new(t.m[3], t.m[7], t.m[11]);
    let failed = || regen_error("DIRECT_EDIT_MOVE_FACE_FAILED");
    let mut results = Vec::new();
    for (b, faces) in groups {
        let solid = model.bodies[b].shape.clone();
        let planes: Vec<Option<occt::Frame>> = faces.iter().map(|f| f.face_plane().ok()).collect();
        // A planar face moved along its own normal is an offset.
        if linear_is_identity
            && faces.len() == 1
            && let Some(plane) = planes[0]
        {
            let along = values::dot(translation, plane.normal);
            if values::norm(values::sub(translation, values::scale(plane.normal, along))) < 1e-12 {
                results.push((b, occt::offset_faces(&solid, &faces, along).map_err(|_| failed())?));
                continue;
            }
        }
        // One planar face turned or moved: replace it with its moved plane.
        if faces.len() == 1
            && let Some(plane) = planes[0]
        {
            let moved = occt::transform(&faces[0], &t).map_err(occt_error)?.shape();
            let target = moved.face_plane().map_err(|_| failed())?;
            debug_assert!(values::norm(plane.normal) > 0.0);
            if let Ok(op) = occt::replace_planar_face(&solid, &faces[0], target) {
                results.push((b, op));
                continue;
            }
        }
        // A hole or boss: heal it away, then cut or add its region moved.
        let healed = occt::delete_faces(&solid, &faces).map_err(|_| failed())?.shape();
        let (healed_volume, volume) = (healed.volume().map_err(occt_error)?, solid.volume().map_err(occt_error)?);
        let subtract = |a: &Shape, b: &Shape| {
            occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(a), std::slice::from_ref(b), true)
        };
        let op = if healed_volume > volume {
            let region = subtract(&healed, &solid).map_err(|_| failed())?.shape();
            let moved = occt::transform(&region, &t).map_err(occt_error)?.shape();
            subtract(&healed, &moved).map_err(|_| failed())?
        } else {
            let region = subtract(&solid, &healed).map_err(|_| failed())?.shape();
            let moved = occt::transform(&region, &t).map_err(occt_error)?.shape();
            occt::boolean(occt::BooleanKind::Union, &[healed], &[moved], true).map_err(|_| failed())?
        };
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

/// `@opOffsetWire`: for thin features, the region between planar edges offset `offset1` to one side and `offset2` to
/// the other (sharp corners), a sheet body per chain of edges.
pub fn offset_wire(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    if values::opt_bool(definition, "makeRegions")? != Some(true) {
        return Err(error("@opOffsetWire: only makeRegions is supported yet"));
    }
    let normal = values::normalize(values::vec3(field(definition, "normal"), "opOffsetWire normal")?)
        .ok_or_else(|| error("@opOffsetWire: zero normal"))?;
    let (mut o1, mut o2) = (
        values::length(field(definition, "offset1"), "opOffsetWire offset1")?,
        values::length(field(definition, "offset2"), "opOffsetWire offset2")?,
    );
    if values::opt_bool(definition, "flip")?.unwrap_or(false) {
        std::mem::swap(&mut o1, &mut o2);
    }
    let found = query::evaluate(model, field(definition, "edges"))?;
    let edges: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Edge).map(|r| model.shape(*r).clone()).collect();
    if edges.is_empty() {
        return Err(regen_error("OFFSET_WIRE_SELECT_WALL_PATH"));
    }
    let info = Info::new(id);
    for wire in occt::connect_edges(&edges).map_err(occt_error)? {
        let region =
            occt::offset_wire_region(&wire, normal, o1, o2).map_err(|_| regen_error("OFFSET_WIRE_GENERIC_ERROR"))?;
        model.add_body(NewBody::new(region, BodyType::Sheet, info.clone()), &Origins::default(), &info, &|_| None);
    }
    Ok(Value::UNDEFINED)
}

/// `@opBodyDraft`: tapers steep faces by adding material between reference edges and the parting object. Each face
/// turns about its reference edge: the given top edges, or for faces (or whole parts), the edge farthest along the
/// pull direction where the face meets a face that isn't steep.
pub fn body_draft(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let pull = values::normalize(values::vec3(field(definition, "pullDirection"), "opBodyDraft pullDirection")?)
        .ok_or_else(|| regen_error("BODY_DRAFT_BAD_PULL_DIRECTION"))?;
    let angle = values::angle(field(definition, "angle"), "opBodyDraft angle")?;
    // Symmetric: the part behind the parting plane is drafted too, against the pull (batch 050).
    let both_sides = values::opt_bool(definition, "bothSides")?.unwrap_or(false);
    let plane_corners = field(definition, "cornerType").as_str() == Some("PLANE");
    // Whole parts can't be drafted on themselves: there's no reference for their faces (batch 051).
    if values::opt_bool(definition, "draftOnSelf")?.unwrap_or(false)
        && field(definition, "selectionType").as_str() == Some("PARTS")
    {
        return Err(regen_error("BODY_DRAFT_INVALID_PART_REFERENCE"));
    }
    // A face is steep if it is closer to the pull direction than the draft angle.
    let normal = |face: &Shape| face.face_tangent_plane(0.5, 0.5).ok().map(|p| p.normal);
    let steep = |face: &Shape| normal(face).is_some_and(|n| values::dot(n, pull).abs() < angle.sin());
    let height = |edge: &Shape| edge.edge_tangent(0.5, false).ok().map(|a| values::dot(a.origin, pull));
    // (body, face, reference edge)
    let mut pairs: Vec<(usize, Shape, Shape)> = Vec::new();
    // Bodies drafted after splitting at the parting plane.
    let mut split_results: Vec<(usize, occt::Operation)> = Vec::new();
    let selection = field(definition, "selectionType").as_str().unwrap_or("EDGES");
    // Symmetric by edges: the steep faces the top and bottom edges bound, drafted as chosen faces are (batch 051).
    let edge_faces: Option<Vec<(usize, Shape)>> = if both_sides && selection == "EDGES" {
        let mut out: Vec<(usize, Shape)> = Vec::new();
        for key in ["topEdges", "bottomEdges"] {
            for r in query::evaluate(model, field(definition, key))? {
                if r.kind != EntityType::Edge {
                    continue;
                }
                let edge = model.shape(r).clone();
                for face in &model.bodies[r.body].faces {
                    let has = face.shape.visible_sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(&edge));
                    if has && steep(&face.shape) && !out.iter().any(|(_, f)| f.is_same(&face.shape)) {
                        out.push((r.body, face.shape.clone()));
                    }
                }
            }
        }
        Some(out)
    } else {
        None
    };
    let selection = if edge_faces.is_some() { "FACES" } else { selection };
    match selection {
        "EDGES" => {
            for r in query::evaluate(model, field(definition, "topEdges"))? {
                if r.kind != EntityType::Edge {
                    continue;
                }
                let edge = model.shape(r).clone();
                let Some(top) = height(&edge) else { continue };
                for face in &model.bodies[r.body].faces {
                    let below = face.shape.bounding_box(false).is_ok_and(|b| {
                        let centre = values::scale(values::add(b.min, b.max), 0.5);
                        values::dot(centre, pull) < top
                    });
                    let has = face.shape.visible_sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(&edge));
                    if has && below && steep(&face.shape) {
                        pairs.push((r.body, face.shape.clone(), edge.clone()));
                    }
                }
            }
        }
        "FACES" | "PARTS" => {
            let mut faces: Vec<(usize, Shape)> = Vec::new();
            if let Some(chosen) = &edge_faces {
                faces = chosen.clone();
            } else if selection == "FACES" {
                for r in query::evaluate(model, field(definition, "faces"))? {
                    if r.kind == EntityType::Face {
                        faces.push((r.body, model.shape(r).clone()));
                    }
                }
            } else {
                let excluded: Vec<Shape> = match field(definition, "excludeFaces") {
                    v if v.is_undefined() => Vec::new(),
                    v => query::evaluate(model, v)?.into_iter().map(|r| model.shape(r).clone()).collect(),
                };
                for b in super::owner_bodies(model, field(definition, "bodies"))? {
                    for face in &model.bodies[b].faces {
                        if steep(&face.shape) && !excluded.iter().any(|e| e.is_same(&face.shape)) {
                            faces.push((b, face.shape.clone()));
                        }
                    }
                }
            }
            // A face's reference edge: its highest edge shared with a face of the solid that isn't steep.
            let reference = |solid: &Shape, face: &Shape| -> Option<Shape> {
                let others: Vec<Shape> =
                    solid.sub_shapes(ShapeKind::Face).into_iter().filter(|f| !f.is_same(face) && !steep(f)).collect();
                face.visible_sub_shapes(ShapeKind::Edge)
                    .into_iter()
                    .filter(|e| others.iter().any(|o| o.sub_shapes(ShapeKind::Edge).iter().any(|x| x.is_same(e))))
                    .filter_map(|e| height(&e).map(|h| (h, e)))
                    .max_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, e)| e)
            };
            // A body the parting plane cuts is split there; only the piece in front of it is drafted, and the
            // pieces are joined again, leaving a ledge at the parting plane. (Batch 007, c019.)
            let parting = parting_plane(model, definition)?;
            // A curved parting sheet (batch 058 c001) splits the body as a plane does: the pieces on the pull side
            // of it are in front.
            let sheet = if parting.is_none() { parting_sheet(model, definition)? } else { None };
            // Both sides of a curved parting sheet, matching at it: each side's faces turn about their far edges,
            // so they meet the sheet at different lines, and Onshape's mitre between them fails (batch 070 c006).
            if both_sides && sheet.is_some() && values::opt_bool(definition, "matchFacesAtParting")?.unwrap_or(false) {
                return Err(regen_error("BODY_DRAFT_MITER_FAILED"));
            }
            let mut bodies: Vec<usize> = faces.iter().map(|(b, _)| *b).collect();
            bodies.dedup();
            for b in bodies {
                let body_faces: Vec<Shape> = faces.iter().filter(|(g, _)| *g == b).map(|(_, f)| f.clone()).collect();
                let shape = model.bodies[b].shape.clone();
                let crossing = parting.filter(|plane| {
                    shape.bounding_box_in(*plane, true).is_ok_and(|bx| bx.min.z < -1e-8 && bx.max.z > 1e-8)
                });
                let curved_split = match (&crossing, &sheet) {
                    (None, Some(sheet)) => {
                        let split = occt::split(std::slice::from_ref(&shape), std::slice::from_ref(sheet))
                            .map_err(|_| regen_error("BODY_DRAFT_FAILED"))?;
                        let pieces = split.shape().sub_shapes(ShapeKind::Solid);
                        (pieces.len() > 1).then_some(pieces)
                    }
                    _ => None,
                };
                if crossing.is_none() && curved_split.is_none() {
                    for face in body_faces {
                        if let Some(edge) = reference(&shape, &face) {
                            pairs.push((b, face, edge));
                        }
                    }
                    continue;
                }
                let (mut front, mut back) = (Vec::new(), Vec::new());
                if let (Some(pieces), Some(sheet)) = (curved_split, &sheet) {
                    for piece in pieces {
                        if side_of_sheet(&piece, sheet, pull)? > 0.0 { front.push(piece) } else { back.push(piece) }
                    }
                } else if let Some(plane) = crossing {
                    let bounds = shape.bounding_box(false).map_err(occt_error)?;
                    let size = values::norm(values::sub(bounds.max, bounds.min)) * 4.0 + 1.0;
                    let cutter = occt::plane_face(plane, -size, size, -size, size).map_err(occt_error)?;
                    let split = occt::split(std::slice::from_ref(&shape), &[cutter])
                        .map_err(|_| regen_error("BODY_DRAFT_FAILED"))?;
                    for piece in split.shape().sub_shapes(ShapeKind::Solid) {
                        let in_front = piece.bounding_box_in(plane, true).is_ok_and(|bx| bx.min.z > -1e-8);
                        if in_front { front.push(piece) } else { back.push(piece) }
                    }
                }
                // Each piece's selected steep faces turn about their edges farthest along the way they're pulled
                // (shared with a face that isn't steep): the piece in front of the parting plane pulled along the
                // pull, the one behind it against it (a symmetric draft keeps the top and bottom, widest at the
                // parting plane, batch 050).
                let draft_piece = |piece: Shape, toward: Vec3| -> R<Shape> {
                    let farthest = |solid: &Shape, face: &Shape| -> Option<Shape> {
                        let others: Vec<Shape> = solid
                            .sub_shapes(ShapeKind::Face)
                            .into_iter()
                            .filter(|f| !f.is_same(face) && !steep(f))
                            .collect();
                        face.visible_sub_shapes(ShapeKind::Edge)
                            .into_iter()
                            .filter(|e| {
                                others.iter().any(|o| o.sub_shapes(ShapeKind::Edge).iter().any(|x| x.is_same(e)))
                            })
                            .filter_map(|e| e.edge_tangent(0.5, false).ok().map(|a| (values::dot(a.origin, toward), e)))
                            .max_by(|a, b| a.0.total_cmp(&b.0))
                            .map(|(_, e)| e)
                    };
                    let mut piece_faces = Vec::new();
                    let mut neutrals = Vec::new();
                    for face in piece.sub_shapes(ShapeKind::Face) {
                        let selected =
                            face.face_inner_point().ok().and_then(|p| occt::vertex(p).ok()).is_some_and(|p| {
                                body_faces.iter().any(|f| p.distance(f).is_ok_and(|d| d.distance < 1e-8))
                            });
                        if !(selected && steep(&face)) {
                            continue;
                        }
                        let Some(edge) = farthest(&piece, &face) else { continue };
                        let at = edge.edge_tangent(0.5, false).map_err(|_| regen_error("BODY_DRAFT_FAILED"))?.origin;
                        neutrals.push(occt::Frame { origin: at, normal: toward, x: values::canonical_x(toward) });
                        piece_faces.push(face);
                    }
                    if piece_faces.is_empty() {
                        return Ok(piece);
                    }
                    let drafted = occt::draft_each(&piece, &piece_faces, toward, angle, &neutrals)
                        .map_err(|_| regen_error("BODY_DRAFT_FAILED"))?
                        .shape();
                    if plane_corners {
                        plane_cut_corners(&piece, &piece_faces, &neutrals, drafted, toward)
                    } else {
                        Ok(drafted)
                    }
                };
                let mut drafted = Vec::new();
                for piece in front {
                    drafted.push(draft_piece(piece, pull)?);
                }
                let back: Vec<Shape> = if both_sides {
                    back.into_iter().map(|piece| draft_piece(piece, values::scale(pull, -1.0))).collect::<R<_>>()?
                } else {
                    back
                };
                let (first, rest) = drafted.split_first().ok_or_else(|| regen_error("BODY_DRAFT_FAILED"))?;
                let tools: Vec<Shape> = rest.iter().cloned().chain(back).collect();
                let op = occt::boolean(occt::BooleanKind::Union, std::slice::from_ref(first), &tools, true)
                    .map_err(|_| regen_error("BODY_DRAFT_FAILED"))?;
                split_results.push((b, op));
            }
        }
        other => return Err(error(format!("@opBodyDraft: selectionType {other} isn't supported"))),
    }
    let mut groups: Vec<(usize, Vec<Shape>, Vec<occt::Frame>)> = Vec::new();
    for (b, face, edge) in pairs {
        let at = edge.edge_tangent(0.5, false).map_err(|_| regen_error("BODY_DRAFT_FAILED"))?.origin;
        let neutral = occt::Frame { origin: at, normal: pull, x: values::canonical_x(pull) };
        match groups.iter_mut().find(|(g, _, _)| *g == b) {
            Some((_, faces, frames)) => {
                if !faces.iter().any(|f| f.is_same(&face)) {
                    faces.push(face);
                    frames.push(neutral);
                }
            }
            None => groups.push((b, vec![face], vec![neutral])),
        }
    }
    if groups.is_empty() && split_results.is_empty() {
        return Err(regen_error("BODY_DRAFT_FAILED"));
    }
    let mut results = split_results;
    for (b, faces, neutrals) in groups {
        let op = occt::draft_each(&model.bodies[b].shape, &faces, pull, angle, &neutrals)
            .map_err(|_| regen_error("BODY_DRAFT_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

/// A drafted piece's corners made planes (`cornerType` `PLANE`, batch 050): where two drafted faces meet at a
/// steep edge, each keeps its side in the other's original plane, and the plane through the edge's end at the
/// reference and those two sides' far ends cuts away what extending the faces to meet left beyond it.
fn plane_cut_corners(
    piece: &Shape,
    faces: &[Shape],
    neutrals: &[occt::Frame],
    drafted: Shape,
    toward: Vec3,
) -> R<Shape> {
    use values::{add, cross, dot, norm, normalize, scale, sub};
    let failed = || regen_error("BODY_DRAFT_FAILED");
    // The point three planes (point, normal) share.
    let meet = |a: (Vec3, Vec3), b: (Vec3, Vec3), c: (Vec3, Vec3)| -> Option<Vec3> {
        let (n1, n2, n3) = (a.1, b.1, c.1);
        let det = dot(n1, cross(n2, n3));
        if det.abs() < 1e-12 {
            return None;
        }
        let (d1, d2, d3) = (dot(n1, a.0), dot(n2, b.0), dot(n3, c.0));
        Some(scale(add(add(scale(cross(n2, n3), d1), scale(cross(n3, n1), d2)), scale(cross(n1, n2), d3)), 1.0 / det))
    };
    let plane_of = |f: &Shape| f.face_plane().ok().map(|p| (p.origin, p.normal));
    let mut cutters = Vec::new();
    let bounds = piece.bounding_box(false).map_err(occt_error)?;
    let size = norm(sub(bounds.max, bounds.min)) * 4.0 + 1e-3;
    for i in 0..faces.len() {
        for j in i + 1..faces.len() {
            let (f1, f2) = (&faces[i], &faces[j]);
            let e1 = f1.sub_shapes(ShapeKind::Edge);
            let Some(edge) = f2.sub_shapes(ShapeKind::Edge).into_iter().find(|e| e1.iter().any(|x| x.is_same(e)))
            else {
                continue;
            };
            let (Ok(a), Ok(b)) = (edge.edge_tangent(0.0, false), edge.edge_tangent(1.0, false)) else { continue };
            let (p_ref, p_far) = if dot(a.origin, toward) >= dot(b.origin, toward) {
                (a.origin, b.origin)
            } else {
                (b.origin, a.origin)
            };
            let (Some(s1), Some(s2)) = (plane_of(f1), plane_of(f2)) else { continue };
            // The drafted faces: those of the result through each face's reference edge.
            let drafted_plane = |k: usize| -> Option<(Vec3, Vec3)> {
                let at = neutrals[k].origin;
                drafted.sub_shapes(ShapeKind::Face).into_iter().find_map(|g| {
                    let on = occt::vertex(at).and_then(|v| v.distance(&g)).is_ok_and(|d| d.distance < 1e-8);
                    let p = plane_of(&g)?;
                    (on && norm(cross(p.1, toward)) > 1e-9 && dot(p.1, (if k == i { s1 } else { s2 }).1) > 0.5)
                        .then_some(p)
                })
            };
            let (Some(d1), Some(d2)) = (drafted_plane(i), drafted_plane(j)) else { continue };
            let far = (p_far, toward);
            let (Some(b1), Some(c1), Some(apex)) = (meet(d1, s2, far), meet(d2, s1, far), meet(d1, d2, far)) else {
                continue;
            };
            let Some(mut n) = normalize(cross(sub(b1, p_ref), sub(c1, p_ref))) else { continue };
            if dot(sub(apex, p_ref), n) < 0.0 {
                n = scale(n, -1.0);
            }
            if dot(sub(apex, p_ref), n) < 1e-9 {
                continue;
            }
            let x = normalize(sub(b1, p_ref)).ok_or_else(failed)?;
            let face = occt::plane_face(occt::Frame { origin: p_ref, normal: n, x }, -size, size, -size, size)
                .map_err(occt_error)?;
            cutters.push(occt::prism(&face, scale(n, size)).map_err(occt_error)?.shape());
        }
    }
    if cutters.is_empty() {
        return Ok(drafted);
    }
    Ok(occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(&drafted), &cutters, true)
        .map_err(|_| failed())?
        .shape())
}

/// The plane of a body draft's parting object, if it is a planar face (or a sheet of one).
/// A body draft's parting object when it is a curved face (or a sheet of one): the face.
fn parting_sheet(model: &Model, definition: &Value) -> R<Option<Shape>> {
    let parting = field(definition, "partingObject");
    if parting.is_undefined() || parting.get("normal").is_some() {
        return Ok(None);
    }
    for r in query::evaluate(model, parting)? {
        let face = match r.kind {
            EntityType::Face => model.shape(r).clone(),
            EntityType::Body => match model.bodies[r.body].faces.as_slice() {
                [only] => only.shape.clone(),
                _ => continue,
            },
            _ => continue,
        };
        if face.face_plane().is_err() {
            return Ok(Some(face));
        }
    }
    Ok(None)
}

/// Which side of a sheet a solid lies: positive on the side the pull direction points to, from the sheet's point
/// nearest a point inside the solid.
fn side_of_sheet(solid: &Shape, sheet: &Shape, pull: Vec3) -> R<f64> {
    let inside = solid.centroid().map_err(occt_error)?;
    let near = occt::vertex(inside).and_then(|v| v.distance(sheet)).map_err(occt_error)?;
    let (fu, fv) = sheet.face_fraction(near.b).map_err(occt_error)?;
    let mut normal = sheet.face_tangent_plane(fu, fv).map_err(occt_error)?.normal;
    if values::dot(normal, pull) < 0.0 {
        normal = values::scale(normal, -1.0);
    }
    Ok(values::dot(values::sub(inside, near.b), normal))
}

fn parting_plane(model: &Model, definition: &Value) -> R<Option<occt::Frame>> {
    let parting = field(definition, "partingObject");
    if parting.is_undefined() {
        return Ok(None);
    }
    if parting.get("normal").is_some() {
        return values::plane(parting, "opBodyDraft partingObject").map(Some);
    }
    for r in query::evaluate(model, parting)? {
        let face = match r.kind {
            EntityType::Face => model.shape(r).clone(),
            EntityType::Body => match model.bodies[r.body].faces.as_slice() {
                [only] => only.shape.clone(),
                _ => continue,
            },
            _ => continue,
        };
        if let Ok(plane) = face.face_plane() {
            return Ok(Some(plane));
        }
    }
    Ok(None)
}

/// A tube's shell, open at both ends: a solid whose two removed faces are parallel planes and whose kept faces run
/// round between them in a ring, each meeting its two neighbours along an edge from one end to the other (batch 141
/// c003: a square lofted to a circle, its sides meeting at the square's corners and running on tangent at the
/// circle, where OCCT's offsets fail). The cavity is built from the kept faces' offsets a thickness in (their
/// surfaces carried on past their edges, as Onshape's walls are): each pair of neighbours' offsets meet along a
/// curve, found point by point at even heights along the ends' normal by damped least squares (steady where they
/// run on tangent), and each wall is fitted through rows at those heights from one such curve to the other, a
/// little past each end, where planes close it. The solid less that cavity, or `None` where it isn't such a tube.
fn tube_shell(solid: &Shape, removed: &[Shape], thickness: f64) -> R<Option<occt::Operation>> {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use values::{dot, scale, sub};
    let depth = -thickness;
    if depth <= 0.0 {
        return Ok(None);
    }
    let [end0, end1] = removed else { return Ok(None) };
    let (Ok(p0), Ok(p1)) = (end0.face_plane(), end1.face_plane()) else { return Ok(None) };
    let Some(axis) = values::normalize(p0.normal) else { return Ok(None) };
    if values::norm(values::cross(axis, p1.normal)) > 1e-9 {
        return Ok(None);
    }
    let (h0, h1) = (dot(p0.origin, axis), dot(p1.origin, axis));
    let (lo, hi) = (h0.min(h1), h0.max(h1));
    let sides: Vec<Shape> =
        solid.sub_shapes(ShapeKind::Face).into_iter().filter(|f| !removed.iter().any(|r| r.is_same(f))).collect();
    if sides.len() < 2 {
        return Ok(None);
    }
    // The ring: each side's two neighbours, each across an edge running from one end to the other.
    let shared = |a: &Shape, b: &Shape| -> Option<Shape> {
        let theirs = b.sub_shapes(ShapeKind::Edge);
        a.sub_shapes(ShapeKind::Edge).into_iter().find(|e| theirs.iter().any(|f| f.is_same(e)))
    };
    let mut ring = vec![0usize];
    let mut corners: Vec<Shape> = Vec::new();
    loop {
        let current = *ring.last().unwrap_or(&0);
        let next = (0..sides.len())
            .filter(|&j| j != current && (ring.len() < 2 || j != ring[ring.len() - 2]))
            .find_map(|j| shared(&sides[current], &sides[j]).map(|edge| (j, edge)));
        let Some((next, edge)) = next else { return Ok(None) };
        corners.push(edge);
        if next == ring[0] {
            break;
        }
        if ring.contains(&next) || ring.len() > sides.len() {
            return Ok(None);
        }
        ring.push(next);
    }
    if ring.len() != sides.len() {
        return Ok(None);
    }
    let n = ring.len();
    let walls: Vec<&Shape> = ring.iter().map(|&i| &sides[i]).collect();
    // Each corner edge runs from one end to the other.
    let height = |p: Vec3| dot(p, axis);
    for e in &corners {
        let (Ok(a), Ok(b)) = (e.edge_tangent(0.0, false), e.edge_tangent(1.0, false)) else { return Ok(None) };
        let (ha, hb) = (height(a.origin), height(b.origin));
        if (ha.min(hb) - lo).abs() > 1e-7 || (ha.max(hb) - hi).abs() > 1e-7 {
            return Ok(None);
        }
    }
    // A wall's offset point at fractions of its face's parameter box (past them, its surface carried on).
    let offset = |w: usize, (fu, fv): (f64, f64)| -> R<Vec3> {
        let plane = walls[w].face_tangent_plane(fu, fv).map_err(occt_error)?;
        Ok(sub(plane.origin, scale(plane.normal, depth)))
    };
    let margin = depth.max((hi - lo) * 0.01) * 0.5;
    let (z_lo, z_hi) = (lo - margin, hi + margin);
    let z_at = |s: f64| z_lo + s * (z_hi - z_lo);
    // Where corner `k`'s two walls' offsets meet at height z: each wall's fractions there.
    type Meeting = ((f64, f64), (f64, f64));
    let solved: RefCell<Vec<BTreeMap<i64, Meeting>>> = RefCell::new(vec![BTreeMap::new(); n]);
    let key = |z: f64| (z * 1e12).round() as i64;
    let meeting = |k: usize, z: f64| -> R<Meeting> {
        if let Some(m) = solved.borrow()[k].get(&key(z)) {
            return Ok(*m);
        }
        let (a, b) = (k, (k + 1) % n);
        // From the nearest meeting found, or from the corner edge's point at that height.
        let nearest = {
            let map = &solved.borrow()[k];
            let below = map.range(..key(z)).next_back().map(|(kk, m)| (key(z) - kk, *m));
            let above = map.range(key(z)..).next().map(|(kk, m)| (kk - key(z), *m));
            match (below, above) {
                (Some(x), Some(y)) => Some(if x.0 <= y.0 { x.1 } else { y.1 }),
                (x, y) => x.or(y).map(|m| m.1),
            }
        };
        let start = match nearest {
            Some(m) => m,
            None => {
                let edge = &corners[k];
                let (mut f0, mut f1) = (0.0, 1.0);
                let h = |f: f64| -> R<f64> { Ok(height(edge.edge_tangent(f, false).map_err(occt_error)?.origin)) };
                let rising = h(1.0)? > h(0.0)?;
                for _ in 0..60 {
                    let m = 0.5 * (f0 + f1);
                    if (h(m)? < z) == rising { f0 = m } else { f1 = m }
                }
                let p = edge.edge_tangent(0.5 * (f0 + f1), false).map_err(occt_error)?.origin;
                (walls[a].face_fraction(p).map_err(occt_error)?, walls[b].face_fraction(p).map_err(occt_error)?)
            }
        };
        let residual = |x: [f64; 4]| -> R<[f64; 4]> {
            let (pa, pb) = (offset(a, (x[0], x[1]))?, offset(b, (x[2], x[3]))?);
            let d = sub(pa, pb);
            Ok([d.x, d.y, d.z, height(pa) - z])
        };
        let mut x = [start.0 .0, start.0 .1, start.1 .0, start.1 .1];
        let mut r = residual(x)?;
        let size = |r: &[f64; 4]| r.iter().map(|v| v * v).sum::<f64>();
        let mut mu = 1e-12;
        for _ in 0..200 {
            if size(&r).sqrt() < 1e-16 {
                break;
            }
            let h = 1e-7;
            let mut jac = [[0.0; 4]; 4];
            for c in 0..4 {
                let mut y = x;
                y[c] += h;
                let ry = residual(y)?;
                for row in 0..4 {
                    jac[row][c] = (ry[row] - r[row]) / h;
                }
            }
            // Damped normal equations: (JᵀJ + μ diag) δ = −Jᵀr.
            let mut improved = false;
            for _ in 0..30 {
                let mut m = [[0.0; 5]; 4];
                for i in 0..4 {
                    for j in 0..4 {
                        m[i][j] = (0..4).map(|row| jac[row][i] * jac[row][j]).sum::<f64>();
                    }
                    m[i][i] += mu * (1.0 + m[i][i]);
                    m[i][4] = -(0..4).map(|row| jac[row][i] * r[row]).sum::<f64>();
                }
                let Some(delta) = solve4(m) else {
                    mu *= 10.0;
                    continue;
                };
                let y = [x[0] + delta[0], x[1] + delta[1], x[2] + delta[2], x[3] + delta[3]];
                let ry = residual(y)?;
                if size(&ry) < size(&r) {
                    x = y;
                    r = ry;
                    mu = (mu * 0.1).max(1e-15);
                    improved = true;
                    break;
                }
                mu *= 10.0;
            }
            if !improved {
                break;
            }
        }
        if size(&r).sqrt() > 1e-11 {
            return Err(regen_error("SHELL_FAILED"));
        }
        let m = ((x[0], x[1]), (x[2], x[3]));
        solved.borrow_mut()[k].insert(key(z), m);
        Ok(m)
    };
    // Wall w's point at height z, a fraction f of the way from its meeting with the wall before it to the next.
    let point = |w: usize, z: f64, f: f64| -> R<Vec3> {
        let left = meeting((w + n - 1) % n, z)?.1;
        let right = meeting(w, z)?.0;
        if f <= 0.0 {
            return offset(w, left);
        }
        if f >= 1.0 {
            return offset(w, right);
        }
        let mut uv = (left.0 + f * (right.0 - left.0), left.1 + f * (right.1 - left.1));
        for _ in 0..60 {
            let p = offset(w, uv)?;
            let miss = z - height(p);
            if miss.abs() < 1e-16 {
                break;
            }
            let h = 1e-7;
            let gu = (height(offset(w, (uv.0 + h, uv.1))?) - height(p)) / h;
            let gv = (height(offset(w, (uv.0, uv.1 + h))?) - height(p)) / h;
            let g2 = gu * gu + gv * gv;
            if g2 < 1e-30 {
                return Err(regen_error("SHELL_FAILED"));
            }
            uv = (uv.0 + gu * miss / g2, uv.1 + gv * miss / g2);
        }
        offset(w, uv)
    };
    const ACROSS: usize = 17;
    const ALONG: usize = 33;
    let across: Vec<f64> = (0..ACROSS).map(|i| i as f64 / (ACROSS - 1) as f64).collect();
    let along: Vec<f64> = (0..ALONG).map(|i| i as f64 / (ALONG - 1) as f64).collect();
    let row = |w: usize, s: f64| -> R<Vec<Vec3>> {
        let z = z_at(s);
        let mut curves = super::surface::sampled_rows_at(1, &across, |f| Ok(vec![point(w, z, f)?]))?;
        Ok(curves.remove(0).poles)
    };
    let mut faces = Vec::new();
    let mut bottoms = Vec::new();
    let mut tops = Vec::new();
    let knots_across = super::surface::sampled_rows_at(1, &across, |f| Ok(vec![Vec3::new(f, 0.0, 0.0)]))?.remove(0).knots;
    for w in 0..n {
        let count = row(w, 0.0)?.len();
        let curves = super::surface::sampled_rows_at(count, &along, |s| row(w, s))?;
        let nv = curves[0].poles.len();
        let poles: Vec<Vec3> = curves.iter().flat_map(|c| c.poles.iter().copied()).collect();
        let face = occt::bspline_surface_face(&poles, (count, nv), &knots_across, &curves[0].knots, (3, 3))
            .map_err(|_| regen_error("SHELL_FAILED"))?;
        faces.push(face);
        let edge = |poles: Vec<Vec3>| occt::bspline_edge(&poles, &[], &knots_across, 3, false).map_err(|_| regen_error("SHELL_FAILED"));
        bottoms.push(edge(curves.iter().map(|c| c.poles[0]).collect())?);
        tops.push(edge(curves.iter().map(|c| c.poles[nv - 1]).collect())?);
    }
    for ends in [&bottoms, &tops] {
        faces.push(occt::fill_edges(ends).map_err(|_| regen_error("SHELL_FAILED"))?);
    }
    let sewn = occt::sew_faces(&faces, 1e-6).map_err(|_| regen_error("SHELL_FAILED"))?;
    let cavity = occt::solid_from(&sewn).map_err(|_| regen_error("SHELL_FAILED"))?;
    // Turned outward where the walls' rows ran the other way round.
    let cavity = if cavity.volume().map_err(occt_error)? < 0.0 { cavity.reversed() } else { cavity };
    let op = occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(solid), &[cavity], false)
        .map_err(|_| regen_error("SHELL_FAILED"))?;
    Ok(Some(op))
}

/// The solution of four linear equations (an augmented 4 × 5 matrix), by elimination with partial pivoting.
fn solve4(mut m: [[f64; 5]; 4]) -> Option<[f64; 4]> {
    for c in 0..4 {
        let p = (c..4).max_by(|&i, &j| m[i][c].abs().total_cmp(&m[j][c].abs()))?;
        if m[p][c].abs() < 1e-300 {
            return None;
        }
        m.swap(c, p);
        for r in 0..4 {
            if r != c {
                let k = m[r][c] / m[c][c];
                for j in c..5 {
                    m[r][j] -= k * m[c][j];
                }
            }
        }
    }
    Some([m[0][4] / m[0][0], m[1][4] / m[1][1], m[2][4] / m[2][2], m[3][4] / m[3][3]])
}
