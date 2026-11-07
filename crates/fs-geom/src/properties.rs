//! Body properties: `@setProperty` and `@getProperty`. The engine keeps every property as given; only `NAME`
//! means anything to it, as the name written to STEP.

use crate::model::Model;
use crate::values::field;
use crate::{GeomBuiltins, model_mut, query};
use fs_interp::Value;
use fs_interp::builtins::arg;
use fs_interp::eval::{R, error};
use std::rc::Rc;

/// The key a property is kept under: its `PropertyType`, or `CUSTOM:<id>` for a custom property.
fn key(definition: &Value) -> R<Rc<str>> {
    let property =
        field(definition, "propertyType").as_str().ok_or_else(|| error("propertyType must be a PropertyType"))?;
    Ok(match property {
        "CUSTOM" => {
            let id = field(definition, "customPropertyId")
                .as_str()
                .ok_or_else(|| error("customPropertyId must be a string"))?;
            format!("CUSTOM:{id}").into()
        }
        other => other.into(),
    })
}

/// The bodies owning what a query finds, in the query's order.
fn bodies(model: &Model, query: &Value) -> R<Vec<usize>> {
    let mut bodies = Vec::new();
    for r in query::evaluate(model, query)? {
        if !bodies.contains(&r.body) {
            bodies.push(r.body);
        }
    }
    Ok(bodies)
}

pub fn register(m: &mut GeomBuiltins) {
    m.insert("setProperty", |_, args| {
        let definition = arg(&args, 1, "setProperty")?.clone();
        let key = key(&definition)?;
        let value = field(&definition, "value").clone();
        model_mut(&args, "setProperty", |model| {
            for body in bodies(model, field(&definition, "entities"))? {
                model.update_body(body, |body| {
                    let properties = Rc::make_mut(&mut body.properties);
                    if value.is_undefined() {
                        properties.remove(&key);
                    } else {
                        properties.insert(key.clone(), value.clone());
                    }
                });
            }
            Ok(Value::UNDEFINED)
        })
    });
    m.insert("getProperty", |_, args| {
        let definition = arg(&args, 1, "getProperty")?.clone();
        let key = key(&definition)?;
        model_mut(&args, "getProperty", |model| {
            let body = bodies(model, field(&definition, "entity"))?.first().copied();
            Ok(body.and_then(|b| model.bodies[b].properties.get(&key).cloned()).unwrap_or(Value::UNDEFINED))
        })
    });
}
