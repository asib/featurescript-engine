//! A FeatureScript interpreter that runs Onshape's std unchanged.
//!
//! [`Engine`] loads modules (std is found in a vendored directory), evaluates expressions and calls functions.
//! Values follow FeatureScript's semantics ([`value`]); builtins live in [`builtins`].
//!
//! The evaluator recurses deeply, so run it on a thread with a large stack ([`run_with_large_stack`]). Type tags
//! are numbered per thread, so use one engine per thread.

// Map keys are `Value`s, and boxes inside them have interior mutability. Boxes order by their creation number,
// which never changes, so keys never move.
#![allow(clippy::mutable_key_type)]

pub mod builtins;
pub mod eval;
pub mod features;
pub mod module;
pub mod types;
pub mod value;

use eval::{Flow, Interp};
use module::{ModuleId, Modules};
use std::fmt;
use std::path::{Path, PathBuf};
pub use value::Value;

/// The FeatureScript language version of the vendored std.
pub const LANGUAGE_VERSION: u32 = 3112;

/// An exception that escaped to the caller: a thrown value or a language error.
#[derive(Clone)]
pub struct Exception(pub Value);

impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.as_str() {
            Some(message) => write!(f, "{message}"),
            None => write!(f, "{:?}", self.0),
        }
    }
}

impl fmt::Debug for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

pub struct Engine {
    pub interp: Interp,
    std_dir: PathBuf,
}

impl Engine {
    pub fn new(std_dir: impl Into<PathBuf>) -> Engine {
        types::reset();
        let std_dir = std_dir.into();
        Engine { interp: Interp::new(Modules::new(std_dir.clone())), std_dir }
    }

    /// An engine over the std vendored in this repository.
    pub fn with_vendored_std() -> Engine {
        Engine::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../std"))
    }

    pub fn std_dir(&self) -> &Path {
        &self.std_dir
    }

    pub fn load_file(&mut self, path: impl AsRef<Path>) -> Result<ModuleId, String> {
        self.interp.modules.load_file(path.as_ref()).map_err(|e| e.0)
    }

    /// Loads a module from text. `name` identifies it; relative imports resolve against the current directory.
    pub fn load_source(&mut self, name: &str, source: &str) -> Result<ModuleId, String> {
        self.interp.modules.load_source(Path::new(name), source).map_err(|e| e.0)
    }

    /// A module importing std's `geometry.fs`, which makes nearly all of std visible.
    pub fn load_geometry(&mut self) -> Result<ModuleId, String> {
        let source = format!(
            "FeatureScript {v};\nimport(path : \"onshape/std/geometry.fs\", version : \"{v}.0\");\n",
            v = LANGUAGE_VERSION
        );
        self.load_source("<geometry>", &source)
    }

    pub fn eval(&mut self, module: ModuleId, expression: &str) -> Result<Value, Exception> {
        let expr = fs_lang::parse_expr(expression).map_err(|e| Exception(Value::string(e.describe(expression))))?;
        unwrap(self.interp.eval_in_module(module, &expr))
    }

    pub fn call(&mut self, module: ModuleId, name: &str, args: Vec<Value>) -> Result<Value, Exception> {
        unwrap(self.interp.call_in_module(module, name, args))
    }

    /// Evaluates every top-level constant in every loaded module, returning `module: name: error` for failures.
    pub fn check_all_constants(&mut self) -> Vec<String> {
        let mut failures = Vec::new();
        for module in 0..self.interp.modules.list.len() {
            let ast = self.interp.modules.list[module].ast.clone();
            for (item, declaration) in ast.items.iter().enumerate() {
                if let fs_lang::ast::ItemKind::Const { name, .. } = &declaration.kind
                    && let Err(e) = unwrap(self.interp.const_value(module, item))
                {
                    let path = &self.interp.modules.list[module].path;
                    let file = Path::new(path).file_name().map_or(path.clone(), |f| f.to_string_lossy().into_owned());
                    failures.push(format!("{file}: {}: {e}", name.name));
                }
            }
        }
        failures
    }
}

fn unwrap(result: Result<Value, Flow>) -> Result<Value, Exception> {
    match result {
        Ok(v) => Ok(v),
        Err(Flow::Throw(v)) => Err(Exception(v)),
        Err(Flow::Return(v)) => Ok(v),
        Err(Flow::Break | Flow::Continue) => Err(Exception(Value::string("break or continue outside a loop"))),
    }
}

/// Runs `f` on a thread with a 512 MB stack, for the evaluator's deep recursion. WebAssembly has no threads here: the
/// module's own stack is made large instead (crates/fs-wasm), and `f` runs on it.
#[cfg(target_os = "emscripten")]
pub fn run_with_large_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    f()
}

/// Runs `f` on a thread with a 512 MB stack, for the evaluator's deep recursion.
#[cfg(not(target_os = "emscripten"))]
pub fn run_with_large_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(f)
        .expect("spawn interpreter thread")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}
