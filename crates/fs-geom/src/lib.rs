//! Geometry for the FeatureScript interpreter: the `@` builtins that std's features call to build and measure
//! models, implemented on OpenCascade through the `occt` crate.
//!
//! [`install`] adds the builtins to an interpreter and gives every new context a [`model::GeomState`]; [`engine`]
//! is an [`Engine`] over the vendored std with geometry installed.

pub mod model;
pub mod ops;
pub mod query;
pub mod sketch;
pub mod values;

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
    interp.state.new_context_state = Some(|| Box::new(GeomState::default()));
    let mut builtins = GeomBuiltins::new();
    query::register(&mut builtins);
    sketch::register(&mut builtins);
    ops::register(&mut builtins);
    interp.builtins.extend(builtins);
}

/// An engine over the vendored std, with geometry.
pub fn engine() -> Engine {
    let mut engine = Engine::with_vendored_std();
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

fn with_context_model<T>(context: &fs_interp::builtins::Context, f: impl FnOnce(&mut Model) -> R<T>) -> R<T> {
    let mut state = context.state.borrow_mut();
    let geometry = state
        .as_mut()
        .and_then(|s| s.as_any_mut().downcast_mut::<GeomState>())
        .ok_or_else(|| error("this context has no geometry"))?;
    f(&mut geometry.model)
}

/// Runs `f` on the model of the context in argument 1.
pub(crate) fn model_mut<T>(args: &[Value], name: &str, f: impl FnOnce(&mut Model) -> R<T>) -> R<T> {
    with_context_model(context_arg(args, 0, name)?, f)
}

/// Runs `f` on the model of a context value.
pub(crate) fn model_of<T>(context: &Value, name: &str, f: impl FnOnce(&mut Model) -> R<T>) -> R<T> {
    let context = match &context.kind {
        Kind::Builtin(native) => native.as_any().downcast_ref::<fs_interp::builtins::Context>(),
        _ => None,
    }
    .ok_or_else(|| error(format!("@{name}: expected a Context")))?;
    with_context_model(context, f)
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

pub(crate) fn occt_error(e: occt::Error) -> Flow {
    error(format!("geometry kernel: {e}"))
}
