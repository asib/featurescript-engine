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
    let face_groups = by_body(model, entities, EntityType::Face)?;
    let transforms =
        field(definition, "transforms").as_array().ok_or_else(|| error("@opPattern: transforms must be an array"))?;
    let names = field(definition, "instanceNames")
        .as_array()
        .ok_or_else(|| error("@opPattern: instanceNames must be an array"))?;
    if names.len() != transforms.len() {
        return Err(error("@opPattern: instanceNames and transforms must be the same size"));
    }
    let copy_properties = values::opt_bool(definition, "copyPropertiesAndAttributes")?.unwrap_or(true);

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
            if copy_properties {
                new.properties = body.properties.clone();
            }
            copies.push((new, info.clone()));
        }
    }
    for (new, info) in copies {
        model.add_body(new, &Origins::default(), &info, &|_| None);
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
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}
