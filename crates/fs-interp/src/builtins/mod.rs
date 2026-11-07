//! Functions provided by the runtime, called from FeatureScript as `@name(...)`.
//!
//! Every builtin goes through [`registry`]. A call to one that isn't written yet throws `not implemented: @name`
//! and is recorded in [`crate::eval::Interp::missing_builtins`].

mod containers;
mod context;
mod math;
mod matrix;
mod regex;
mod strings;

use crate::eval::{Interp, R, error};
use crate::value::{Value, ValueMap};
use std::collections::HashMap;
use std::rc::Rc;

pub use context::{Context, ContextState, context_arg};

pub type BuiltinFn = fn(&mut Interp, Vec<Value>) -> R<Value>;

/// State builtins keep per engine.
#[derive(Default)]
pub struct State {
    /// Makes the [`ContextState`] each new context starts with.
    pub new_context_state: Option<fn() -> Box<dyn ContextState>>,
    /// State another crate's builtins keep per engine.
    pub extension: Option<Box<dyn std::any::Any>>,
}

pub fn registry() -> HashMap<&'static str, BuiltinFn> {
    let mut map: HashMap<&'static str, BuiltinFn> = HashMap::new();
    math::register(&mut map);
    containers::register(&mut map);
    strings::register(&mut map);
    matrix::register(&mut map);
    context::register(&mut map);
    regex::register(&mut map);
    map
}

// ----- Argument helpers, also for builtins in other crates -----

pub fn arg<'a>(args: &'a [Value], i: usize, name: &str) -> R<&'a Value> {
    args.get(i).ok_or_else(|| error(format!("@{name}: missing argument {}", i + 1)))
}

pub fn number(args: &[Value], i: usize, name: &str) -> R<f64> {
    let v = arg(args, i, name)?;
    v.as_number()
        .ok_or_else(|| error(format!("@{name}: argument {} must be a number, not {}", i + 1, v.type_description())))
}

pub fn string<'a>(args: &'a [Value], i: usize, name: &str) -> R<&'a str> {
    let v = arg(args, i, name)?;
    v.as_str()
        .ok_or_else(|| error(format!("@{name}: argument {} must be a string, not {}", i + 1, v.type_description())))
}

pub fn array<'a>(args: &'a [Value], i: usize, name: &str) -> R<&'a Rc<Vec<Value>>> {
    let v = arg(args, i, name)?;
    v.as_array()
        .ok_or_else(|| error(format!("@{name}: argument {} must be an array, not {}", i + 1, v.type_description())))
}

pub fn map<'a>(args: &'a [Value], i: usize, name: &str) -> R<&'a Rc<ValueMap>> {
    let v = arg(args, i, name)?;
    v.as_map().ok_or_else(|| error(format!("@{name}: argument {} must be a map, not {}", i + 1, v.type_description())))
}

pub fn index(args: &[Value], i: usize, name: &str) -> R<usize> {
    let n = number(args, i, name)?;
    if n < 0.0 || n.fract() != 0.0 {
        return Err(error(format!("@{name}: argument {} must be a non-negative integer, not {n}", i + 1)));
    }
    Ok(n as usize)
}

/// A number result, rejecting NaN (FeatureScript has none).
pub fn checked(name: &str, n: f64) -> R<Value> {
    if n.is_nan() { Err(error(format!("@{name}: invalid numeric operation"))) } else { Ok(Value::number(n)) }
}

pub fn map_value(entries: Vec<(&str, Value)>) -> Value {
    Value::map(entries.into_iter().map(|(k, v)| (Value::string(k), v)).collect())
}
