//! `@opMergeContexts`: brings another context's bodies into this one, as instantiating and deriving Part Studios
//! do (std's `derive`, used by `frame` with a profile sketch, `importDerived` and others).

use std::collections::HashMap;
use std::rc::Rc;

use crate::model::{Body, EntityRef, EntityType, FeatureId, Info, Model, NewBody, Origins, Tid};
use crate::query::{self, tid_string};
use crate::values::{self, field};
use fs_interp::Value;
use fs_interp::value::Kind;
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
        // And its attributes (sheet metal walls and joints, batch 023).
        let mut origins = Origins::default();
        for entity in body.all_entities() {
            if !entity.attributes.is_empty() {
                origins.attributes_from(&entity.shape, &entity.attributes);
            }
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
            infos
                .get(&s.hash())
                .and_then(|list| list.iter().find(|(shape, _)| shape.is_same(s)).map(|(_, i)| i.clone()))
        };
        let mut new = NewBody::new(body.shape.clone(), body.body_type, info.clone());
        new.construction = body.construction;
        new.sketch_plane = body.sketch_plane;
        new.frame = body.frame;
        new.attributes = body.attributes.clone();
        new.owner = body.owner;
        new.properties = if clear_custom {
            Rc::new(
                body.properties
                    .iter()
                    .filter(|(k, _)| !k.starts_with("CUSTOM:"))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            )
        } else {
            body.properties.clone()
        };
        model.add_body(new, &origins, &info, &special);
    }
    // Owners point at the copies of the bodies they pointed at.
    let copies: Vec<(crate::model::Tid, crate::model::Tid)> =
        source.bodies.iter().enumerate().map(|(i, b)| (b.tid, model.bodies[first + i].tid)).collect();
    for i in 0..source.bodies.len() {
        if let Some(owner) = model.bodies[first + i].owner {
            let mapped = copies.iter().find(|(old, _)| *old == owner).map(|(_, new)| *new);
            model.update_body(first + i, |b| b.owner = mapped);
        }
    }
    // Sheet metal attributes name entities and bodies by transient id (the association between a model's definition
    // and its parts, the tools registered on its walls): they name the copies now. Two copies of one source would
    // otherwise share their association, and rebuilding one model would replace the other's parts (batch 023).
    let mut renamed: HashMap<Tid, Tid> = HashMap::new();
    for (i, body) in source.bodies.iter().enumerate() {
        let copy = model.bodies[first + i].clone();
        renamed.insert(body.tid, copy.tid);
        for kind in [EntityType::Face, EntityType::Edge, EntityType::Vertex] {
            for entity in body.entities(kind) {
                if let Some(c) = copy.entities(kind).iter().find(|c| c.shape.is_same(&entity.shape)) {
                    renamed.insert(entity.tid, c.tid);
                }
            }
        }
    }
    for b in first..model.bodies.len() {
        let body = model.bodies[b].clone();
        let mut refs = vec![EntityRef { body: b, kind: EntityType::Body, index: 0 }];
        for kind in [EntityType::Face, EntityType::Edge, EntityType::Vertex] {
            refs.extend((0..body.entities(kind).len()).map(|index| EntityRef { body: b, kind, index }));
        }
        for r in refs {
            if model.attributes(r).unnamed.iter().any(is_sheet_metal) {
                model.update_attributes(r, |attributes| {
                    for a in attributes.unnamed.iter_mut().filter(|a| is_sheet_metal(a)) {
                        *a = rename_tids(a, &renamed);
                    }
                });
            }
        }
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

fn is_sheet_metal(attribute: &Value) -> bool {
    attribute.tag.is_some_and(|t| matches!(&*fs_interp::types::name(t), "SMAttribute" | "SMAssociationAttribute"))
}

/// A value with the transient ids in it (`T12`, or a transient query's string) renamed.
fn rename_tids(value: &Value, renamed: &HashMap<Tid, Tid>) -> Value {
    const QUERY: (&str, &str) = ("{ queryType : TRANSIENT , transientId : ", " }");
    let rename = |s: &str| -> Option<String> {
        if let Some(t) = query::parse_tid(s) {
            return renamed.get(&t).map(|n| tid_string(*n));
        }
        let t = s.strip_prefix(QUERY.0)?.strip_suffix(QUERY.1).and_then(query::parse_tid)?;
        renamed.get(&t).map(|n| format!("{}{}{}", QUERY.0, tid_string(*n), QUERY.1))
    };
    let kind = match &value.kind {
        Kind::String(s) => match rename(s) {
            Some(n) => Kind::String(n.into()),
            None => return value.clone(),
        },
        Kind::Array(items) => Kind::Array(Rc::new(items.iter().map(|v| rename_tids(v, renamed)).collect())),
        Kind::Map(entries) => {
            Kind::Map(Rc::new(entries.iter().map(|(k, v)| (k.clone(), rename_tids(v, renamed))).collect()))
        }
        _ => return value.clone(),
    };
    Value { tag: value.tag, kind }
}
