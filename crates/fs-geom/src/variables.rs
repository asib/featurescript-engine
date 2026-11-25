//! Context variables (`@setVariable`, `@getVariable` and the query-variable builtins). They live in the
//! context's state, so a failed feature's variables roll back with its geometry.

use crate::model::Variable;
use crate::values::{field, map};
use crate::{GeomBuiltins, state_mut, state_ref};
use fs_interp::Value;
use fs_interp::builtins::arg;
use fs_interp::eval::{R, error};
use fs_interp::value::ValueMap;
use std::rc::Rc;

fn name_of(definition: &Value, builtin: &str) -> R<Rc<str>> {
    field(definition, "name").as_str().map(Rc::from).ok_or_else(|| error(format!("@{builtin}: name must be a string")))
}

fn set(args: Vec<Value>, builtin: &'static str, query: bool) -> R<Value> {
    let definition = arg(&args, 1, builtin)?.clone();
    let name = name_of(&definition, builtin)?;
    let variable = Variable {
        value: field(&definition, "value").clone(),
        description: field(&definition, "description").clone(),
        query,
    };
    state_mut(&args, builtin, |state| {
        Rc::make_mut(&mut state.variables).insert(name, variable);
        Ok(Value::UNDEFINED)
    })
}

fn get(args: Vec<Value>, builtin: &'static str, query: bool) -> R<Value> {
    let definition = arg(&args, 1, builtin)?.clone();
    let name = name_of(&definition, builtin)?;
    let default = definition.get("defaultValue").cloned();
    state_ref(&args, builtin, |state| match state.variables.get(&name) {
        Some(v) if v.query == query => Ok(v.value.clone()),
        _ => default.ok_or_else(|| error(format!("Variable \"{name}\" not found"))),
    })
}

fn all(args: Vec<Value>, builtin: &'static str, descriptions: bool) -> R<Value> {
    state_ref(&args, builtin, |state| {
        let entries: ValueMap = state
            .variables
            .iter()
            .filter(|(_, v)| !v.query)
            .map(|(name, v)| {
                let value = if descriptions {
                    map(vec![("value", v.value.clone()), ("description", v.description.clone())])
                } else {
                    v.value.clone()
                };
                (Value::string(name.clone()), value)
            })
            .collect();
        Ok(Value::map(entries))
    })
}

pub fn register(m: &mut GeomBuiltins) {
    m.insert("setVariable", |_, args| set(args, "setVariable", false));
    m.insert("getVariable", |_, args| get(args, "getVariable", false));
    m.insert("setQueryVariable", |_, args| set(args, "setQueryVariable", true));
    m.insert("getQueryVariable", |_, args| get(args, "getQueryVariable", true));
    m.insert("getAllVariables", |_, args| all(args, "getAllVariables", false));
    m.insert("getAllVariablesAndDescriptions", |_, args| all(args, "getAllVariablesAndDescriptions", true));
}
