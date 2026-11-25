//! Body properties: `@setProperty` and `@getProperty`. The engine keeps every property as given; only `NAME`
//! means anything to it, as the name written to STEP. A material comes back with its quantities as plain SI
//! numbers, which std's `getProperty` gives units again.

use crate::model::Model;
use crate::values::field;
use crate::{GeomBuiltins, model_mut, model_ref, query};
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

/// A map with each quantity (a `ValueWithUnits`) in it replaced by its SI number.
fn without_units(material: &Value) -> Value {
    let Some(entries) = material.as_map() else { return material.clone() };
    let mut out = entries.as_ref().clone();
    for value in out.values_mut() {
        if let (Some(number), Some(_)) = (value.get("value").and_then(Value::as_number), value.get("unit")) {
            *value = Value::number(number);
        }
    }
    Value::map(out).with_tag(material.tag)
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
        // Onshape won't read a context's properties while a feature in it is running (batch 011, the hole table).
        if fs_interp::builtins::regenerating(fs_interp::builtins::context_arg(&args, 0, "getProperty")?) {
            return Err(fs_interp::eval::execution_error(
                "@getProperty: cannot get properties during feature regeneration in current context",
            ));
        }
        let definition = arg(&args, 1, "getProperty")?.clone();
        let key = key(&definition)?;
        model_ref(&args, "getProperty", |model| {
            let body = bodies(model, field(&definition, "entity"))?.first().copied();
            let value = body.and_then(|b| model.bodies[b].properties.get(&key).cloned()).unwrap_or(Value::UNDEFINED);
            Ok(if &*key == "MATERIAL" { without_units(&value) } else { value })
        })
    });
}
