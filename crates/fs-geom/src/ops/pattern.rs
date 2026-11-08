//! `@opPattern`: transformed copies of bodies.

use super::{owner_bodies, transform};
use crate::model::{EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, field};
use crate::{occt_error, query};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use std::rc::Rc;

pub fn pattern(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let entities = field(definition, "entities");
    if query::evaluate(model, entities)?.iter().any(|r| r.kind != EntityType::Body) {
        return Err(error("@opPattern: patterning faces isn't supported yet"));
    }
    let bodies = owner_bodies(model, entities)?;
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
    Ok(Value::UNDEFINED)
}
