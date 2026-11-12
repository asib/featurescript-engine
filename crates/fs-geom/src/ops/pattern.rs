//! `@opPattern`: transformed copies of bodies, and of the bosses and pockets faces of a body make.

use super::{by_body, replace_bodies, transform};
use crate::model::{EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, field};
use crate::{occt_error, query, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use std::rc::Rc;

pub fn pattern(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let entities = field(definition, "entities");
    let mut bodies: Vec<usize> =
        query::evaluate(model, entities)?.into_iter().filter(|r| r.kind == EntityType::Body).map(|r| r.body).collect();
    bodies.dedup();
    // A composite part is copied with the bodies in it, the copies making a composite part of their own (std's cut
    // list copies a frame's closed composite segment so, batch 035).
    let composites: Vec<usize> = bodies.iter().copied().filter(|&b| model.bodies[b].composite.is_some()).collect();
    bodies.retain(|&b| model.bodies[b].composite.is_none());
    for &c in &composites {
        if let Some((members, _)) = &model.bodies[c].composite {
            for m in model.composite_members(members) {
                if !bodies.contains(&m) {
                    bodies.push(m);
                }
            }
        }
    }
    let mut face_groups = by_body(model, entities, EntityType::Face)?;
    // Faces of sheets (a plane, say) are copied as they are, each copy a sheet of its own instance.
    let sheet_faces: Vec<(usize, Vec<occt::Shape>)> = face_groups
        .iter()
        .filter(|(b, _)| model.bodies[*b].body_type == crate::model::BodyType::Sheet)
        .cloned()
        .collect();
    face_groups.retain(|(b, _)| model.bodies[*b].body_type != crate::model::BodyType::Sheet);
    let transforms =
        field(definition, "transforms").as_array().ok_or_else(|| error("@opPattern: transforms must be an array"))?;
    let names = field(definition, "instanceNames")
        .as_array()
        .ok_or_else(|| error("@opPattern: instanceNames must be an array"))?;
    if names.len() != transforms.len() {
        return Err(error("@opPattern: instanceNames and transforms must be the same size"));
    }
    let copy_properties = values::opt_bool(definition, "copyPropertiesAndAttributes")?.unwrap_or(true);
    // A hole's faces and profile edges copied as the same hole (std copying sheet metal hole tools to register them)
    // or as a new one (patterning them), batch 032: the names `@opHole` gave them carry over.
    let holes = field(definition, "holePropagationType").as_str().map(String::from);

    let mut copies = Vec::new();
    // Which body each copy copies, by instance, for composite parts' copies.
    let mut sources: Vec<(usize, Option<usize>)> = Vec::new();
    for (instance, (t, name)) in transforms.iter().zip(names.iter()).enumerate() {
        let t = transform(t)?;
        let name = name.as_str().ok_or_else(|| error("@opPattern: instance names must be strings"))?;
        let mut instance_id: Vec<Rc<str>> = id.to_vec();
        instance_id.push(Rc::from(name));
        let info = Info::new(instance_id.into());
        for &b in &bodies {
            let body = model.bodies[b].clone();
            let op = occt::transform(&body.shape, &t).map_err(occt_error)?;
            let mut new = NewBody::new(op.shape(), body.body_type, info.clone());
            new.construction = body.construction;
            new.frame = body.frame.map(|f| super::transform_frame(&f, &t));
            // A sketch's sheet copied is still a sketch's (an aligned section copies one, batch 037).
            new.sketch_plane = body.sketch_plane.map(|f| super::transform_frame(&f, &t));
            // Each copy is made from the body it copies, for tracking (std finds patterned form tools so, batch 033).
            new.ancestry = crate::model::Ancestry::copy_of_body(&body);
            // Each copy is tracked back to what it copies, and takes its attributes too unless told not to.
            let mut origins = Origins::default();
            for entity in body.all_entities() {
                for image in op.modified(&entity.shape) {
                    match (entity.info.label.is_some(), holes.as_deref()) {
                        (true, Some("PROPAGATE_SAME_HOLE")) => origins.inherit(&image, entity),
                        (true, Some("PROPAGATE_NEW_HOLE")) => {
                            let copied = Rc::new(Info {
                                created_by: info.created_by.clone(),
                                label: entity.info.label.clone(),
                                identity: entity.info.identity,
                                ..Default::default()
                            });
                            origins.copy_with_info(&image, entity, copied);
                        }
                        _ if copy_properties => origins.copy(&image, entity),
                        _ => origins.derive(&image, entity, true),
                    }
                }
            }
            if copy_properties {
                new.properties = body.properties.clone();
                new.attributes = body.attributes.clone();
            }
            copies.push((new, info.clone(), origins, Some((body.clone(), op.shape(), t.clone()))));
            sources.push((instance, Some(b)));
        }
    }
    for (t, name) in transforms.iter().zip(names.iter()) {
        let t = transform(t)?;
        let name = name.as_str().ok_or_else(|| error("@opPattern: instance names must be strings"))?;
        let mut instance_id: Vec<Rc<str>> = id.to_vec();
        instance_id.push(Rc::from(name));
        let info = Info::new(instance_id.into());
        for (b, faces) in &sheet_faces {
            let body = model.bodies[*b].clone();
            let sheet = occt::extract_faces(faces, 0.0).map_err(occt_error)?;
            for piece in sheet {
                let op = occt::transform(&piece, &t).map_err(occt_error)?;
                let mut new = NewBody::new(op.shape(), body.body_type, info.clone());
                new.construction = body.construction;
                copies.push((new, info.clone(), Origins::default(), None));
                sources.push((usize::MAX, None));
            }
        }
    }
    let mut added = Vec::new();
    for (new, info, origins, moved) in copies {
        let b = model.add_body(new, &origins, &info, &|_| None);
        if let Some((source, shape, t)) = moved {
            super::carry_frames(model, b, &source, &shape, &t);
        }
        added.push(b);
    }
    for (instance, name) in names.iter().enumerate() {
        let name = name.as_str().ok_or_else(|| error("@opPattern: instance names must be strings"))?;
        let mut instance_id: Vec<Rc<str>> = id.to_vec();
        instance_id.push(Rc::from(name));
        let info = Info::new(instance_id.into());
        for &c in &composites {
            let composite = model.bodies[c].clone();
            let Some((members, closed)) = &composite.composite else { continue };
            let member_bodies = model.composite_members(members);
            let copied: Vec<usize> = sources
                .iter()
                .zip(&added)
                .filter(|((i, source), _)| *i == instance && source.is_some_and(|s| member_bodies.contains(&s)))
                .map(|(_, &a)| a)
                .collect();
            let shapes: Vec<occt::Shape> = copied.iter().map(|&a| model.bodies[a].shape.clone()).collect();
            let mut new = NewBody::new(occt::compound(&shapes), crate::model::BodyType::Composite, info.clone());
            new.composite = Some((copied.iter().map(|&a| model.bodies[a].tid).collect(), *closed));
            new.ancestry = crate::model::Ancestry::copy_of_body(&composite);
            if copy_properties {
                new.properties = composite.properties.clone();
                new.attributes = composite.attributes.clone();
            }
            model.add_body(new, &Origins::default(), &info, &|_| None);
        }
    }
    // Faces: the boss (or pocket) they make, capped where it meets the rest of the body, is added to (or cut from)
    // the body at each transform.
    // Each set of faces joined by edges makes its own tool: two holes a feature cut are two pockets (batch 139
    // c006: a sketch's two circles cut through and patterned).
    let mut results = Vec::new();
    for (b, faces) in face_groups {
        let mut components: Vec<Vec<occt::Shape>> = Vec::new();
        for face in faces {
            let edges = face.sub_shapes(occt::ShapeKind::Edge);
            let touching: Vec<usize> = components
                .iter()
                .enumerate()
                .filter(|(_, c)| {
                    c.iter().any(|g| g.sub_shapes(occt::ShapeKind::Edge).iter().any(|e| edges.iter().any(|f| f.is_same(e))))
                })
                .map(|(i, _)| i)
                .collect();
            let mut joined = vec![face];
            for &i in touching.iter().rev() {
                joined.extend(components.remove(i));
            }
            components.push(joined);
        }
        let (mut pockets, mut bosses) = (Vec::new(), Vec::new());
        for faces in &components {
            let tool = occt::face_feature_solid(faces).map_err(|_| regen_error("PATTERN_FACE_FAILED"))?;
            // Just outside the first face is inside the tool only for a pocket's void.
            let plane = faces[0].face_tangent_plane(0.5, 0.5).map_err(occt_error)?;
            let pocket =
                tool.contains_point(values::add(plane.origin, values::scale(plane.normal, 1e-6))).map_err(occt_error)?;
            for t in transforms.iter() {
                let moved = occt::transform(&tool, &transform(t)?).map_err(occt_error)?.shape();
                if pocket { pockets.push(moved) } else { bosses.push(moved) }
            }
        }
        let (kind, tools) = match (bosses.is_empty(), pockets.is_empty()) {
            (true, true) => continue,
            (false, true) => (occt::BooleanKind::Union, bosses),
            (true, false) => (occt::BooleanKind::Subtraction, pockets),
            (false, false) => {
                return Err(error("@opPattern: faces making both bosses and pockets aren't supported yet"));
            }
        };
        let op = occt::boolean(kind, std::slice::from_ref(&model.bodies[b].shape), &tools, true)
            .map_err(|_| regen_error("PATTERN_FACE_FAILED"))?;
        results.push((b, op));
    }
    // Edges of a sheet bounding a hole in one of its faces (a sheet metal face pattern of a hole, batch 017): the
    // hole is cut again at each transform, in the face's plane.
    let edge_groups: Vec<(usize, Vec<occt::Shape>)> = by_body(model, entities, EntityType::Edge)?
        .into_iter()
        .filter(|(b, _)| model.bodies[*b].body_type == crate::model::BodyType::Sheet)
        .collect();
    // How each sheet faced, to keep it so: a boolean of a sheet whose shell is turned over as a whole (as an extrude's
    // sheets are) gives its faces back the shell's own way (batch 141 c004: a sheet metal channel's definition, its
    // material then rebuilt on the other side).
    let mut facing: Vec<(crate::model::Tid, occt::Vec3, occt::Vec3)> = Vec::new();
    for (b, edges) in edge_groups {
        let body = &model.bodies[b];
        if let Some(t) = body.faces.first().and_then(|f| f.shape.face_tangent_plane(0.5, 0.5).ok()) {
            facing.push((body.tid, t.origin, t.normal));
        }
        // The edges by the face they bound: a hole cut through two walls of a sheet metal box is a hole in each
        // (batch 143 c001).
        let mut by_face: Vec<(usize, Vec<occt::Shape>)> = Vec::new();
        for edge in &edges {
            let Some(f) = body.faces.iter().position(|f| f.shape.sub_shapes(occt::ShapeKind::Edge).iter().any(|x| x.is_same(edge)))
            else {
                return Err(error("@opPattern: patterned edges must bound a face"));
            };
            match by_face.iter_mut().find(|(g, _)| *g == f) {
                Some((_, list)) => list.push(edge.clone()),
                None => by_face.push((f, vec![edge.clone()])),
            }
        }
        let mut tools = Vec::new();
        for (f, edges) in &by_face {
            let frame = body.faces[*f]
                .shape
                .face_plane()
                .map_err(|_| error("@opPattern: patterned edges must lie in a planar face"))?;
            for region in occt::sketch_regions(edges, frame).map_err(occt_error)? {
                let below = occt::transform(&region, &super::translation(values::scale(frame.normal, -1.0)))
                    .map_err(occt_error)?
                    .shape();
                let tool = occt::prism(&below, values::scale(frame.normal, 2.0)).map_err(occt_error)?.shape();
                for t in transforms.iter() {
                    tools.push(occt::transform(&tool, &transform(t)?).map_err(occt_error)?.shape());
                }
            }
        }
        let op = occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(&body.shape), &tools, false)
            .map_err(|_| regen_error("PATTERN_FACE_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    for (tid, at, normal) in facing {
        let Some(b) = model.bodies.iter().position(|body| body.tid == tid) else { continue };
        let Ok(vertex) = occt::vertex(at) else { continue };
        let Some(face) = model.bodies[b].faces.iter().find(|f| vertex.distance(&f.shape).is_ok_and(|d| d.distance < 1e-7))
        else {
            continue;
        };
        let Some(now) = face.shape.face_fraction(at).ok().and_then(|(u, v)| face.shape.face_tangent_plane(u, v).ok()) else {
            continue;
        };
        if values::dot(now.normal, normal) < 0.0 {
            model.update_body(b, |body| {
                body.shape = body.shape.reversed();
                for f in body.faces.iter_mut() {
                    f.shape = f.shape.reversed();
                }
            });
        }
    }
    Ok(Value::UNDEFINED)
}

/// `@opBooleanedPattern` (internal in std): copies of the tool bodies by each transform, joined to (or cut from, or
/// intersected with) the target body; the tools themselves stay (batch 102 c005). Its definition wants a target
/// body, tool bodies and an operation type, asked for in that order.
pub fn booleaned_pattern(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let bodies = |model: &Model, key: &str| -> R<Vec<usize>> {
        match field(definition, key) {
            v if v.is_undefined() => Ok(Vec::new()),
            v => super::owner_bodies(model, v),
        }
    };
    let targets = bodies(model, "targetBody")?;
    let &[target] = targets.as_slice() else { return Err(crate::faulty("CANNOT_RESOLVE_ENTITIES", "targetBody")) };
    let tools = bodies(model, "toolBodies")?;
    if tools.is_empty() {
        return Err(crate::faulty("CANNOT_RESOLVE_ENTITIES", "toolBodies"));
    }
    let kind = match field(definition, "operationType").as_str() {
        Some("UNION") => occt::BooleanKind::Union,
        Some("SUBTRACTION") => occt::BooleanKind::Subtraction,
        Some("INTERSECTION") => occt::BooleanKind::Intersection,
        _ => return Err(crate::faulty("INVALID_INPUT", "operationType")),
    };
    let transforms = field(definition, "transforms").as_array().ok_or_else(|| error("transforms must be an array"))?.clone();
    let mut copies = Vec::new();
    for t in transforms.iter() {
        let t = transform(t)?;
        for &b in &tools {
            copies.push(occt::transform(&model.bodies[b].shape, &t).map_err(occt_error)?.shape());
        }
    }
    let op = occt::boolean(kind, std::slice::from_ref(&model.bodies[target].shape), &copies, true)
        .map_err(|_| regen_error("BOOLEAN_INVALID"))?;
    replace_bodies(model, &id, vec![(target, op)]);
    Ok(Value::UNDEFINED)
}
