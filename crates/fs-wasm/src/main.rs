//! The FeatureScript engine for WebAssembly (Emscripten), its entry points working on files in the module's virtual
//! file system, std preloaded at /std (see the library, src/lib.rs, for what they do).

use std::ffi::{CStr, c_char};

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

fn main() {}
