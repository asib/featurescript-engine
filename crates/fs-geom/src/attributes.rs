//! Attributes: data set on bodies and their entities (`@setAttribute`, `@getAttributes`, `@getAttribute`,
//! `@getAllAttributes`, `@removeAttributes`), and the attribute filter query.

use std::rc::Rc;

use crate::model::{Attributes, EntityRef, Model, same_type};
use crate::values::field;
use crate::{GeomBuiltins, model_mut, query};
use fs_interp::Value;
use fs_interp::builtins::arg;
use fs_interp::eval::error;

/// What to look for: a named attribute (perhaps with an exact value, or matching a pattern), or legacy unnamed
/// attributes of a pattern's type.
pub struct Wanted {
    name: Option<Rc<str>>,
    exact: Option<Value>,
    pattern: Option<Value>,
}

impl Wanted {
    pub fn of(definition: &Value) -> Wanted {
        let optional = |key: &str| Some(field(definition, key).clone()).filter(|v| !v.is_undefined());
        Wanted {
            name: field(definition, "name").as_str().map(Rc::from),
            exact: optional("valueToMatchExactly"),
            pattern: optional("attributePattern"),
        }
    }

    /// The attributes of `attributes` that match.
    pub fn matching<'a>(&self, attributes: &'a Attributes) -> Vec<&'a Value> {
        match &self.name {
            Some(name) => attributes
                .named
                .get(name)
                .filter(|v| self.exact.as_ref().is_none_or(|e| *v == e))
                .filter(|v| self.pattern.as_ref().is_none_or(|p| fields_match(v, p)))
                .into_iter()
                .collect(),
            None => attributes
                .unnamed
                .iter()
                .filter(|v| self.pattern.as_ref().is_none_or(|p| same_type(v, p) && fields_match(v, p)))
                .collect(),
        }
    }
}

/// Whether every entry of a map pattern is in the value with the same value. A pattern that isn't a map matches
/// anything (of its type).
fn fields_match(value: &Value, pattern: &Value) -> bool {
    let Some(pattern) = pattern.as_map() else { return true };
    let Some(value) = value.as_map() else { return pattern.is_empty() };
    pattern.iter().all(|(k, v)| value.get(k) == Some(v))
}

/// The entities of a query (with `undefined` meaning everything) that have a matching attribute.
pub fn filter(model: &Model, found: Vec<EntityRef>, wanted: &Wanted) -> Vec<EntityRef> {
    found.into_iter().filter(|r| !wanted.matching(model.attributes(*r)).is_empty()).collect()
}

/// The distinct attributes matching `wanted` on what the definition's `entities` find.
fn get_attributes(args: &[Value], name: &str, definition: &Value, wanted: &Wanted) -> fs_interp::eval::R<Value> {
    model_mut(args, name, |model| {
        let mut out: Vec<Value> = Vec::new();
        for r in query::evaluate(model, field(definition, "entities"))? {
            for value in wanted.matching(model.attributes(r)) {
                if !out.contains(value) {
                    out.push(value.clone());
                }
            }
        }
        Ok(Value::array(out))
    })
}

pub fn register(m: &mut GeomBuiltins) {
    m.insert("setAttribute", |_, args| {
        let definition = arg(&args, 1, "setAttribute")?.clone();
        let name = field(&definition, "name").as_str().map(Rc::<str>::from);
        let value = field(&definition, "attribute").clone();
        model_mut(&args, "setAttribute", |model| {
            let entities = field(&definition, "entities");
            let found = query::evaluate(model, entities)?;
            if found.is_empty() {
                // Flat patterns aren't modelled, so their counterparts of a part's faces are nothing to attribute
                // (std attributes a sheet metal hole's flat faces too, batch 032).
                if field(entities, "queryType").as_str() == Some("CORRESPONDING_IN_FLAT") {
                    return Ok(Value::UNDEFINED);
                }
                return Err(error("@setAttribute: entities resolve to nothing"));
            }
            for r in found {
                let mut clash = false;
                model.update_attributes(r, |attributes| match &name {
                    Some(name) if value.is_undefined() => {
                        attributes.named.remove(name);
                    }
                    Some(name) => {
                        attributes.named.insert(name.clone(), value.clone());
                    }
                    None if attributes.unnamed.iter().any(|v| same_type(v, &value)) => clash = true,
                    None => attributes.unnamed.push(value.clone()),
                });
                if clash {
                    return Err(error("@setAttribute: an unnamed attribute of that type is already set"));
                }
            }
            Ok(Value::UNDEFINED)
        })
    });
    m.insert("getAttributes", |_, args| {
        let definition = arg(&args, 1, "getAttributes")?.clone();
        get_attributes(&args, "getAttributes", &definition, &Wanted::of(&definition))
    });
    // The holes' own attributes, unnamed `HoleAttribute`s (what the hole table reads).
    m.insert("getHoleAttributes", |_, args| {
        let definition = arg(&args, 1, "getHoleAttributes")?.clone();
        let tag =
            fs_interp::types::find("HoleAttribute").ok_or_else(|| error("std type HoleAttribute isn't loaded"))?;
        let pattern = Value::map(Default::default()).with_tag(Some(tag));
        get_attributes(
            &args,
            "getHoleAttributes",
            &definition,
            &Wanted { name: None, exact: None, pattern: Some(pattern) },
        )
    });
    m.insert("getAttribute", |_, args| {
        let definition = arg(&args, 1, "getAttribute")?.clone();
        let name = field(&definition, "name").as_str().unwrap_or_default().to_string();
        model_mut(&args, "getAttribute", |model| {
            let first = query::evaluate(model, field(&definition, "entity"))?.first().copied();
            Ok(first.and_then(|r| model.attributes(r).named.get(name.as_str()).cloned()).unwrap_or(Value::UNDEFINED))
        })
    });
    m.insert("getAllAttributes", |_, args| {
        let definition = arg(&args, 1, "getAllAttributes")?.clone();
        model_mut(&args, "getAllAttributes", |model| {
            let first = query::evaluate(model, field(&definition, "entity"))?.first().copied();
            let entries = first
                .map(|r| model.attributes(r).named.iter().map(|(k, v)| (Value::string(k.clone()), v.clone())).collect())
                .unwrap_or_default();
            Ok(Value::map(entries))
        })
    });
    m.insert("removeAttributes", |_, args| {
        let definition = arg(&args, 1, "removeAttributes")?.clone();
        let pattern = Some(field(&definition, "attributePattern").clone()).filter(|v| !v.is_undefined());
        model_mut(&args, "removeAttributes", |model| {
            let found = match field(&definition, "entities") {
                v if v.is_undefined() => model.everything(None),
                v => query::evaluate(model, v)?,
            };
            for r in found {
                model.update_attributes(r, |attributes| {
                    attributes
                        .unnamed
                        .retain(|v| pattern.as_ref().is_some_and(|p| !(same_type(v, p) && fields_match(v, p))));
                });
            }
            Ok(Value::UNDEFINED)
        })
    });
}
