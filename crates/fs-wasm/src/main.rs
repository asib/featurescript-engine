//! The FeatureScript engine for WebAssembly (Emscripten), its entry points working on files in the module's virtual
//! file system, std preloaded at /std (see the library, src/lib.rs, for what they do).
//!
//! `fs_build` builds a feature and writes all of it out. `fs_model` only builds it, and keeps it for `fs_mesh`,
//! `fs_step` and `fs_volumes` to write out one at a time, so a page can show a coarse mesh before the rest is made.

use std::cell::RefCell;
use std::ffi::{CStr, c_char};

thread_local! {
    /// The feature `fs_model` last built.
    static MODEL: RefCell<Option<fs_wasm::Model>> = const { RefCell::new(None) };
}

/// Text from a NUL-terminated UTF-8 string.
fn text(p: *const c_char) -> String {
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

/// Writes the features of the Feature Studio at `path` and the parameters of `feature` (or of its first feature, if
/// empty) to `out` as JSON, or the error there as text. Returns 0, or 1 on an error.
///
/// # Safety
/// The arguments are NUL-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fs_parameters(path: *const c_char, feature: *const c_char, out: *const c_char) -> i32 {
    let (path, feature, out) = (text(path), text(feature), text(out));
    let result = std::fs::read_to_string(&path)
        .map_err(|e| format!("{path}: {e}"))
        .and_then(|source| fs_wasm::parameters_json("/std", &path, &source, &feature));
    let (code, written) = match result {
        Ok(json) => (0, json),
        Err(e) => (1, e),
    };
    let _ = std::fs::write(out, written);
    code
}

/// Builds `feature` (or the first feature, if empty) of the Feature Studio at `path` with `values` (a JSON object of
/// parameter values) and writes `<stem>.stl`, `<stem>.step` and `<stem>.log` (what it printed and each part's
/// volume, or the error). Returns 0, or 1 on an error.
///
/// # Safety
/// The arguments are NUL-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fs_build(
    path: *const c_char,
    feature: *const c_char,
    values: *const c_char,
    stem: *const c_char,
) -> i32 {
    let (path, feature, values, stem) = (text(path), text(feature), text(values), text(stem));
    let result = std::fs::read_to_string(&path)
        .map_err(|e| format!("{path}: {e}"))
        .and_then(|source| fs_wasm::build("/std", &path, &source, &feature, &values, &stem));
    let (code, log) = match result {
        Ok(log) => (0, log),
        Err(e) => (1, e),
    };
    let _ = std::fs::write(format!("{stem}.log"), log);
    code
}

/// Builds `feature` (or the first feature, if empty) of the Feature Studio at `path` with `values` (a JSON object of
/// parameter values) and keeps it, in place of the last, for `fs_mesh`, `fs_step` and `fs_volumes`. Writes what it
/// printed, or the error, to `out`. Returns 0, or 1 on an error, which leaves no model kept.
///
/// # Safety
/// The arguments are NUL-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fs_model(
    path: *const c_char,
    feature: *const c_char,
    values: *const c_char,
    out: *const c_char,
) -> i32 {
    let (path, feature, values, out) = (text(path), text(feature), text(values), text(out));
    // The last model's shapes go before the next is built, not after: both needn't be in memory at once.
    MODEL.with(|m| *m.borrow_mut() = None);
    let result = std::fs::read_to_string(&path)
        .map_err(|e| format!("{path}: {e}"))
        .and_then(|source| fs_wasm::model("/std", &path, &source, &feature, &values));
    let (code, written) = match result {
        Ok(model) => {
            let output = model.output.clone();
            MODEL.with(|m| *m.borrow_mut() = Some(model));
            (0, output)
        }
        Err(e) => (1, e),
    };
    let _ = std::fs::write(out, written);
    code
}

const NO_MODEL: &str = "No model is built.";

/// Runs `f` on the model `fs_model` kept and writes its bytes to `out`, or the error there as text. Returns 0, or 1
/// on an error.
fn write_from_model(out: &str, f: impl FnOnce(&fs_wasm::Model) -> Result<Vec<u8>, String>) -> i32 {
    let result = MODEL.with(|m| match m.borrow().as_ref() {
        Some(model) => f(model),
        None => Err(NO_MODEL.to_string()),
    });
    let (code, written) = match result {
        Ok(bytes) => (0, bytes),
        Err(e) => (1, e.into_bytes()),
    };
    let _ = std::fs::write(out, written);
    code
}

/// Writes the kept model to `out` as a binary STL in millimetres, meshed to within `tolerance` (mm), or the error
/// there as text. Returns 0, or 1 on an error.
///
/// # Safety
/// `out` is a NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fs_mesh(tolerance: f64, out: *const c_char) -> i32 {
    write_from_model(&text(out), |model| model.stl(tolerance))
}

/// Writes the kept model to `out` as a STEP file, or the error there as text. Returns 0, or 1 on an error.
///
/// # Safety
/// `out` is a NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fs_step(out: *const c_char) -> i32 {
    let out = text(out);
    let result = MODEL.with(|m| match m.borrow().as_ref() {
        Some(model) => model.write_step(&out),
        None => Err(NO_MODEL.to_string()),
    });
    match result {
        Ok(()) => 0,
        Err(e) => {
            let _ = std::fs::write(out, e);
            1
        }
    }
}

/// Writes each part of the kept model's volume to `out`, a line each, or the error. Returns 0, or 1 on an error.
///
/// # Safety
/// `out` is a NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fs_volumes(out: *const c_char) -> i32 {
    write_from_model(&text(out), |model| model.volumes().map(String::into_bytes))
}

fn main() {}
