//! `@opMergeContexts`: brings another context's bodies into this one, as instantiating and deriving Part Studios
//! do (std's `derive`, used by `frame` with a profile sketch, `importDerived` and others).

use std::collections::HashMap;
use std::rc::Rc;

use crate::model::{Body, EntityRef, FeatureId, Info, Model, NewBody, Origins};
use crate::query::{self, tid_string};
use crate::values::{self, field};
use fs_interp::Value;
use fs_interp::eval::R;
use occt::Shape;

/// What the other context holds: its bodies, and what each tracked query finds there.
pub struct Source {
    pub bodies: Vec<Rc<Body>>,
    pub tracked: Vec<Vec<EntityRef>>,
}

/// Reads the source context before merging.
pub fn read(other: &Model, definition: &Value) -> R<Source> {
    let mut tracked = Vec::new();
    for q in field(definition, "trackThroughMerge").as_array().map(|a| a.to_vec()).unwrap_or_default() {
        tracked.push(query::evaluate(other, &q)?);
    }
    Ok(Source { bodies: other.bodies.clone(), tracked })
}

/// Copies the source's bodies in, created by this operation (keeping what each entity was, such as its sketch
/// entity), and returns, for each tracked query, the transient ids of what it found.
pub fn merge(model: &mut Model, id: FeatureId, definition: &Value, source: &Source) -> R<Value> {
    let clear_custom = values::opt_bool(definition, "clearCustomProperties")?.unwrap_or(false);
    let info = Info::new(id.clone());
    let first = model.bodies.len();
    for body in &source.bodies {
        // Each entity keeps what it was, made by this operation.
        let mut infos: HashMap<u64, Vec<(Shape, Rc<Info>)>> = HashMap::new();
        for entity in body.all_entities() {
            let copy = Rc::new(Info {
                created_by: id.clone(),
                sketch_entity: entity.info.sketch_entity.clone(),
                cap: entity.info.cap,
                label: entity.info.label.clone(),
                surface_frame: entity.info.surface_frame,
                ..Default::default()
            });
            infos.entry(entity.shape.hash()).or_default().push((entity.shape.clone(), copy));
        }
        let special = |s: &Shape| {
            infos.get(&s.hash()).and_then(|list| list.iter().find(|(shape, _)| shape.is_same(s)).map(|(_, i)| i.clone()))
        };
        let mut new = NewBody::new(body.shape.clone(), body.body_type, info.clone());
        new.construction = body.construction;
        new.sketch_plane = body.sketch_plane;
        new.frame = body.frame;
        new.attributes = body.attributes.clone();
        new.properties = if clear_custom {
            Rc::new(body.properties.iter().filter(|(k, _)| !k.starts_with("CUSTOM:")).map(|(k, v)| (k.clone(), v.clone())).collect())
        } else {
            body.properties.clone()
        };
        model.add_body(new, &Origins::default(), &info, &special);
    }
    let results = source
        .tracked
        .iter()
        .map(|found| {
            let queries = found
                .iter()
                .map(|r| {
                    let copy = EntityRef { body: first + r.body, ..*r };
                    Value::string(tid_string(model.tid(copy)))
                })
                .collect();
            Value::array(queries)
        })
        .collect();
    Ok(Value::array(results))
}
