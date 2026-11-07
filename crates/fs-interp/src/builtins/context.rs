//! Contexts and FeatureScript versions.

use super::{BuiltinFn, arg};
use crate::eval::{R, error};
use crate::types;
use crate::value::{Kind, Native, Value, ValueMap, next_serial};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// The modelling context: everything a Part Studio holds.
pub struct Context {
    serial: u64,
    /// The `FeatureScriptVersionNumber` the context runs at, as the enum value's name.
    pub version: RefCell<Rc<str>>,
    /// Feature bookkeeping: open features and reported statuses.
    pub features: RefCell<Features>,
    /// State another crate keeps in the context, such as its geometry. Aborting a feature restores it.
    pub state: RefCell<Option<Box<dyn ContextState>>>,
}

/// State kept in a [`Context`] that features roll back: `@startFeature` takes a snapshot, and `@abortFeature`
/// restores it.
pub trait ContextState: Any {
    fn snapshot(&self) -> Box<dyn Any>;
    fn restore(&mut self, snapshot: Box<dyn Any>);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

#[derive(Default)]
pub struct Features {
    next_token: u64,
    /// Features started and not yet ended or aborted, innermost last.
    pub open: Vec<OpenFeature>,
    /// Status reported for each feature id, as std's builtin status map.
    pub statuses: ValueMap,
    /// Computed parameters (`@setFeatureComputedParameter`): feature id to a map of name to value. Like
    /// statuses, they survive a feature being aborted.
    pub computed: ValueMap,
}

pub struct OpenFeature {
    pub id: Value,
    pub token: u64,
    snapshot: Option<Box<dyn Any>>,
}

fn ok_status() -> Value {
    super::map_value(vec![("statusType", Value::string("OK")), ("statusEnum", Value::string("NO_ERROR"))])
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

impl Context {
    /// Records a feature's status, as `@functionReportFeatureStatus` does. `status_enum` names an
    /// `ErrorStringEnum` value.
    pub fn report_status(&self, id: &Value, status_type: &str, status_enum: &str) {
        let status = super::map_value(vec![
            ("statusType", Value::string(status_type)),
            ("statusEnum", Value::string(status_enum)),
        ]);
        self.features.borrow_mut().statuses.insert(id.clone().with_tag(None), status);
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
    m.insert("newContext", |interp, args| {
        let version = arg(&args, 0, "newContext")?;
        let name: Rc<str> = version.as_str().ok_or_else(|| error("@newContext: expected a version"))?.into();
        Ok(Value::builtin(Rc::new(Context {
            serial: next_serial(),
            version: RefCell::new(name),
            features: RefCell::new(Features::default()),
            state: RefCell::new(interp.state.new_context_state.map(|new| new())),
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
    // ----- Feature bookkeeping -----
    m.insert("startFeature", |_, args| {
        let context = context_arg(&args, 0, "startFeature")?;
        let id = arg(&args, 1, "startFeature")?.clone();
        let snapshot = context.state.borrow().as_ref().map(|state| state.snapshot());
        let mut features = context.features.borrow_mut();
        features.next_token += 1;
        let token = features.next_token;
        features.open.push(OpenFeature { id, token, snapshot });
        // std keeps the token in `var token is map`.
        Ok(super::map_value(vec![("token", Value::number(token as f64))]))
    });
    // The token is the map @startFeature returned, or `{}` meaning the open feature with this id. Ending or
    // aborting a feature also closes the features opened inside it.
    fn finish(args: &[Value], name: &str, abort: bool) -> R<Value> {
        let context = context_arg(args, 0, name)?;
        let id = arg(args, 1, name)?;
        let token = arg(args, 2, name)?.get("token").and_then(Value::as_number).map(|t| t as u64);
        let mut features = context.features.borrow_mut();
        let position = match token {
            Some(token) => features.open.iter().rposition(|f| f.token == token),
            None => features.open.iter().rposition(|f| &f.id == id),
        };
        let Some(i) = position else {
            return Err(error(format!("@{name}: no open feature for {id:?}")));
        };
        let snapshot = features.open.drain(i..).next().and_then(|f| f.snapshot);
        drop(features);
        if abort && let (Some(state), Some(snapshot)) = (context.state.borrow_mut().as_mut(), snapshot) {
            state.restore(snapshot);
        }
        Ok(Value::UNDEFINED)
    }
    m.insert("endFeature", |_, args| finish(&args, "endFeature", false));
    m.insert("abortFeature", |_, args| finish(&args, "abortFeature", true));
    m.insert("functionGetFeatureStatus", |_, args| {
        let context = context_arg(&args, 0, "functionGetFeatureStatus")?;
        let id = arg(&args, 1, "functionGetFeatureStatus")?.clone().with_tag(None);
        Ok(context.features.borrow().statuses.get(&id).cloned().unwrap_or_else(ok_status))
    });
    m.insert("functionReportFeatureStatus", |_, args| {
        let context = context_arg(&args, 0, "functionReportFeatureStatus")?;
        let id = arg(&args, 1, "functionReportFeatureStatus")?.clone().with_tag(None);
        let status = arg(&args, 2, "functionReportFeatureStatus")?.clone().with_tag(None);
        context.features.borrow_mut().statuses.insert(id, status);
        Ok(Value::UNDEFINED)
    });
    m.insert("clearFeatureStatus", |_, args| {
        let context = context_arg(&args, 0, "clearFeatureStatus")?;
        let id = arg(&args, 1, "clearFeatureStatus")?.clone().with_tag(None);
        context.features.borrow_mut().statuses.remove(&id);
        Ok(Value::UNDEFINED)
    });
    m.insert("setFeatureComputedParameter", |_, args| {
        let context = context_arg(&args, 0, "setFeatureComputedParameter")?;
        let id = arg(&args, 1, "setFeatureComputedParameter")?.clone().with_tag(None);
        let definition = arg(&args, 2, "setFeatureComputedParameter")?;
        let name = definition.get("name").cloned().unwrap_or(Value::UNDEFINED);
        let value = definition.get("value").cloned().unwrap_or(Value::UNDEFINED);
        let mut features = context.features.borrow_mut();
        let mut parameters =
            features.computed.get(&id).and_then(Value::as_map).map(|m| (**m).clone()).unwrap_or_default();
        if value.is_undefined() {
            parameters.remove(&name);
        } else {
            parameters.insert(name, value);
        }
        features.computed.insert(id, Value::map(parameters));
        Ok(Value::UNDEFINED)
    });
    // Only used to highlight entities in Onshape's UI.
    m.insert("setErrorEntities", |_, _| Ok(Value::UNDEFINED));
    // Records which queries a feature's parameters used, for Onshape's UI.
    m.insert("recordQuery", |_, _| Ok(Value::UNDEFINED));

    // The language version of the calling module; std's modules all report std's version.
    m.insert("getLanguageVersion", |_, _| Ok(Value::number(crate::LANGUAGE_VERSION as f64)));
}
