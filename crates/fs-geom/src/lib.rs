//! Geometry for the FeatureScript interpreter: the `@` builtins that std's features call to build and measure
//! models, implemented on OpenCascade through the `occt` crate.
//!
//! [`install`] adds the builtins to an interpreter and gives every new context a [`model::GeomState`]; [`engine`]
//! is an [`Engine`] over the vendored std with geometry installed.

// Values are keys of FeatureScript maps; their interior mutability is only in builtin objects, never hashed.
#![allow(clippy::mutable_key_type)]

pub mod attributes;
pub mod bspline;
pub mod compressed;
pub mod curve_extend;
pub mod evaluate;
pub mod evaluate_more;
pub mod font;
pub mod mate;
pub mod model;
pub mod ops;
pub mod paths;
pub mod pattern_stack;
pub mod patterns;
pub mod properties;
pub mod query;
pub mod reuse;
pub mod selection;
pub mod sketch;
pub mod splines;
pub mod topology;
pub mod unmodelled;
pub mod uploads;
pub mod values;
pub mod variables;

use fs_interp::Engine;
use fs_interp::Value;
use fs_interp::builtins::{BuiltinFn, context_arg};
use fs_interp::eval::{Flow, Interp, R, error};
use fs_interp::value::Kind;
use model::{GeomState, Model};
use std::collections::HashMap;
use std::rc::Rc;
use values::Tags;

pub(crate) type GeomBuiltins = HashMap<&'static str, BuiltinFn>;

/// Adds the geometry builtins to `interp`, replacing nothing it already has except where noted in each module.
pub fn install(interp: &mut Interp) {
    reuse::reset();
    interp.state.new_context_state = Some(|| Box::new(GeomState::default()));
    let mut builtins = GeomBuiltins::new();
    query::register(&mut builtins);
    compressed::register(&mut builtins);
    sketch::register(&mut builtins);
    ops::register(&mut builtins);
    properties::register(&mut builtins);
    attributes::register(&mut builtins);
    evaluate::register(&mut builtins);
    evaluate_more::register(&mut builtins);
    bspline::register(&mut builtins);
    unmodelled::register(&mut builtins);
    pattern_stack::register(&mut builtins);
    variables::register(&mut builtins);
    patterns::register(&mut builtins);
    mate::register(&mut builtins);
    paths::register(&mut builtins);
    interp.builtins.extend(builtins);
}

/// An engine over the vendored std, with geometry.
pub fn engine() -> Engine {
    let mut engine = Engine::with_vendored_std();
    install(&mut engine.interp);
    engine
}

/// An engine with the geometry builtins over std in `std_dir` (WebAssembly's virtual file system, say).
pub fn engine_with_std(std_dir: impl Into<std::path::PathBuf>) -> Engine {
    let mut engine = Engine::new(std_dir);
    install(&mut engine.interp);
    engine
}

/// State the geometry builtins keep per engine.
struct Extension {
    tags: Rc<Tags>,
}

/// std's type tags, found the first time a builtin needs them (std has loaded by then).
pub(crate) fn tags(interp: &mut Interp) -> R<Rc<Tags>> {
    if let Some(extension) = interp.state.extension.as_ref().and_then(|e| e.downcast_ref::<Extension>()) {
        return Ok(extension.tags.clone());
    }
    let tags = Rc::new(Tags::find()?);
    interp.state.extension = Some(Box::new(Extension { tags: tags.clone() }));
    Ok(tags)
}

pub(crate) fn with_context_state<T>(
    context: &fs_interp::builtins::Context,
    f: impl FnOnce(&mut GeomState) -> R<T>,
) -> R<T> {
    let mut state = context.state.borrow_mut();
    let geometry = state
        .as_mut()
        .and_then(|s| s.as_any_mut().downcast_mut::<GeomState>())
        .ok_or_else(|| error("this context has no geometry"))?;
    f(geometry)
}

/// Runs `f` on the state (model and variables) of the context in argument 1, which it changes: the state's stamp
/// takes in the call and its arguments. A call that only reads uses [`state_ref`].
pub(crate) fn state_mut<T>(args: &[Value], name: &str, f: impl FnOnce(&mut GeomState) -> R<T>) -> R<T> {
    with_context_state(context_arg(args, 0, name)?, |state| {
        reuse::changed(state, name, &args[1..], false);
        f(state)
    })
}

/// Reads the state of the context in argument 1.
pub(crate) fn state_ref<T>(args: &[Value], name: &str, f: impl FnOnce(&GeomState) -> R<T>) -> R<T> {
    with_context_state(context_arg(args, 0, name)?, |state| f(state))
}

/// Runs `f` on the model of the context in argument 1, which it changes: the state's stamp takes in the call and
/// its arguments. A call that only reads uses [`model_ref`].
pub(crate) fn model_mut<T>(args: &[Value], name: &str, f: impl FnOnce(&mut Model) -> R<T>) -> R<T> {
    let context = context_arg(args, 0, name)?;
    let sheet_metal = fs_interp::builtins::in_sheet_metal(context);
    with_context_state(context, |state| {
        reuse::changed(state, name, &args[1..], sheet_metal);
        state.model.sheet_metal_scope = sheet_metal;
        f(&mut state.model)
    })
}

/// Reads the model of the context in argument 1.
pub(crate) fn model_ref<T>(args: &[Value], name: &str, f: impl FnOnce(&Model) -> R<T>) -> R<T> {
    read_model(context_arg(args, 0, name)?, f)
}

/// Reads the model of the context in argument 1 for a value that depends on nothing but the model and the call's
/// arguments, and reads without altering: a value that took a while to find is remembered for the same call on the
/// same state ([`reuse::read`]).
pub(crate) fn model_read(args: &[Value], name: &str, f: impl FnOnce(&Model) -> R<Value>) -> R<Value> {
    let context = context_arg(args, 0, name)?;
    let sheet_metal = fs_interp::builtins::in_sheet_metal(context);
    with_context_state(context, |state| {
        state.model.sheet_metal_scope = sheet_metal;
        reuse::read(state, name, &args[1..], sheet_metal, |state| f(&state.model))
    })
}

fn read_model<T>(context: &fs_interp::builtins::Context, f: impl FnOnce(&Model) -> R<T>) -> R<T> {
    let sheet_metal = fs_interp::builtins::in_sheet_metal(context);
    with_context_state(context, |state| {
        // Not a change to the state: every call sets it for itself.
        state.model.sheet_metal_scope = sheet_metal;
        f(&state.model)
    })
}

pub(crate) fn context_of<'a>(context: &'a Value, name: &str) -> R<&'a fs_interp::builtins::Context> {
    match &context.kind {
        Kind::Builtin(native) => native.as_any().downcast_ref::<fs_interp::builtins::Context>(),
        _ => None,
    }
    .ok_or_else(|| error(format!("@{name}: expected a Context")))
}

/// Reads the model of a context value.
pub(crate) fn model_of<T>(context: &Value, name: &str, f: impl FnOnce(&Model) -> R<T>) -> R<T> {
    read_model(context_of(context, name)?, f)
}

/// Reads the model of a context value, or `None` if the value isn't a context with geometry.
pub fn with_model<T>(context: &Value, f: impl FnOnce(&Model) -> T) -> Option<T> {
    let mut f = Some(f);
    model_of(context, "with_model", |model| Ok(f.take().map(|f| f(model)))).ok().flatten()
}

/// A library error, as std's `regenError` makes: a map whose `message` is an `ErrorStringEnum` value.
pub(crate) fn regen_error(message: &str) -> Flow {
    let enum_tag = fs_interp::types::find("ErrorStringEnum");
    Flow::Throw(values::map(vec![("message", Value::string(message).with_tag(enum_tag))]))
}

/// A library error naming the definition's field at fault, as std's `regenError` with faulty parameters.
pub(crate) fn faulty(message: &str, parameter: &str) -> Flow {
    let enum_tag = fs_interp::types::find("ErrorStringEnum");
    Flow::Throw(values::map(vec![
        ("message", Value::string(message).with_tag(enum_tag)),
        ("faultyParameters", Value::array(vec![Value::string(parameter)])),
    ]))
}

pub(crate) fn occt_error(e: occt::Error) -> Flow {
    error(format!("geometry kernel: {e}"))
}
