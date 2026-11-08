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
    for (t, name) in transforms.iter().zip(names.iter()) {
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
            copies.push((new, info.clone(), origins));
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
                copies.push((new, info.clone(), Origins::default()));
            }
        }
    }
    for (new, info, origins) in copies {
        model.add_body(new, &origins, &info, &|_| None);
    }
    // Faces: the boss (or pocket) they make, capped where it meets the rest of the body, is added to (or cut from)
    // the body at each transform.
    let mut results = Vec::new();
    for (b, faces) in face_groups {
        let tool = occt::face_feature_solid(&faces).map_err(|_| regen_error("PATTERN_FACE_FAILED"))?;
        // Just outside the first face is inside the tool only for a pocket's void.
        let plane = faces[0].face_tangent_plane(0.5, 0.5).map_err(occt_error)?;
        let pocket =
            tool.contains_point(values::add(plane.origin, values::scale(plane.normal, 1e-6))).map_err(occt_error)?;
        let mut tools = Vec::new();
        for t in transforms.iter() {
            tools.push(occt::transform(&tool, &transform(t)?).map_err(occt_error)?.shape());
        }
        let kind = if pocket { occt::BooleanKind::Subtraction } else { occt::BooleanKind::Union };
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
    for (b, edges) in edge_groups {
        let body = &model.bodies[b];
        let face = body
            .faces
            .iter()
            .find(|f| {
                let own = f.shape.sub_shapes(occt::ShapeKind::Edge);
                edges.iter().all(|e| own.iter().any(|x| x.is_same(e)))
            })
            .ok_or_else(|| error("@opPattern: patterned edges must bound one face"))?;
        let frame = face.shape.face_plane().map_err(|_| error("@opPattern: patterned edges must lie in a planar face"))?;
        let mut tools = Vec::new();
        for region in occt::sketch_regions(&edges, frame).map_err(occt_error)? {
            let below = occt::transform(&region, &super::translation(values::scale(frame.normal, -1.0)))
                .map_err(occt_error)?
                .shape();
            let tool = occt::prism(&below, values::scale(frame.normal, 2.0)).map_err(occt_error)?.shape();
            for t in transforms.iter() {
                tools.push(occt::transform(&tool, &transform(t)?).map_err(occt_error)?.shape());
            }
        }
        let op = occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(&body.shape), &tools, false)
            .map_err(|_| regen_error("PATTERN_FACE_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}
