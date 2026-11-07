//! Contexts and FeatureScript versions.

use super::{BuiltinFn, arg};
use crate::eval::{R, error};
use crate::types;
use crate::value::{Kind, Native, Value, next_serial};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// The modelling context: everything a Part Studio holds. Geometry state is added with the native layer.
pub struct Context {
    serial: u64,
    /// The `FeatureScriptVersionNumber` the context runs at, as the enum value's name.
    pub version: RefCell<Rc<str>>,
}

impl Native for Context {
    fn type_name(&self) -> &'static str {
        "Context"
    }
    fn serial(&self) -> u64 {
        self.serial
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn context_arg<'a>(args: &'a [Value], i: usize, name: &str) -> R<&'a Context> {
    let v = arg(args, i, name)?;
    match &v.kind {
        Kind::Builtin(native) => native.as_any().downcast_ref::<Context>(),
        _ => None,
    }
    .ok_or_else(|| error(format!("@{name}: argument {} must be a Context", i + 1)))
}

/// The declaration index of a `FeatureScriptVersionNumber` value.
fn version_index(v: &Value) -> R<usize> {
    let tag = v.tag.ok_or_else(|| error("expected a FeatureScriptVersionNumber"))?;
    v.as_str().and_then(|s| types::enum_index(tag, s)).ok_or_else(|| error("expected a FeatureScriptVersionNumber"))
}

pub fn register(m: &mut HashMap<&'static str, BuiltinFn>) {
    m.insert("newContext", |_, args| {
        let version = arg(&args, 0, "newContext")?;
        let name: Rc<str> = version.as_str().ok_or_else(|| error("@newContext: expected a version"))?.into();
        Ok(Value::builtin(Rc::new(Context {
            serial: next_serial(),
            version: RefCell::new(name),
        })))
    });
    m.insert("isContext", |_, args| {
        let v = arg(&args, 0, "isContext")?;
        Ok(Value::bool(matches!(&v.kind, Kind::Builtin(n) if n.as_any().is::<Context>())))
    });
    m.insert("getCurrentVersion", |_, args| {
        let context = context_arg(&args, 0, "getCurrentVersion")?;
        Ok(Value::string(context.version.borrow().clone()))
    });
    m.insert("isAtVersionOrLater", |_, args| {
        let context = context_arg(&args, 0, "isAtVersionOrLater")?;
        let introduced = arg(&args, 1, "isAtVersionOrLater")?;
        let tag = introduced.tag.ok_or_else(|| error("@isAtVersionOrLater: expected a FeatureScriptVersionNumber"))?;
        let current = types::enum_index(tag, &context.version.borrow())
            .ok_or_else(|| error("@isAtVersionOrLater: the context's version is unknown"))?;
        Ok(Value::bool(version_index(introduced)? <= current))
    });
    // The language version of the calling module; std's modules all report std's version.
    m.insert("getLanguageVersion", |_, _| Ok(Value::number(crate::LANGUAGE_VERSION as f64)));
}
