//! Emscripten link settings for the WebAssembly engine: exported entry points, a large stack for the evaluator's
//! recursion (WebAssembly runs it without threads), growable memory, WebAssembly exceptions as OCCT's are, and std
//! preloaded into the virtual file system at /std.

use std::env;
use std::path::Path;

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("emscripten") {
        return;
    }
    let std_dir = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../std").canonicalize().unwrap();
    for arg in [
        "-fwasm-exceptions".to_string(),
        // Rust links through emcc, which leaves out C++'s runtime unless asked (the bridge and OCCT need it).
        "-lc++".to_string(),
        "-lc++abi".to_string(),
        "-sEXPORTED_FUNCTIONS=['_fs_build','_fs_parameters','_fs_model','_fs_mesh','_fs_step','_fs_volumes','_main']"
            .to_string(),
        "-sEXPORTED_RUNTIME_METHODS=['ccall','cwrap','FS','HEAPU8']".to_string(),
        "-sMODULARIZE=1".to_string(),
        "-sEXPORT_ES6=1".to_string(),
        "-sEXPORT_NAME=createFeatureScriptEngine".to_string(),
        "-sENVIRONMENT=web,worker,node".to_string(),
        "-sALLOW_MEMORY_GROWTH=1".to_string(),
        "-sMAXIMUM_MEMORY=4GB".to_string(),
        "-sSTACK_SIZE=64MB".to_string(),
        "-sINITIAL_MEMORY=128MB".to_string(),
        format!("--preload-file={}@/std", std_dir.display()),
    ] {
        println!("cargo:rustc-link-arg-bins={arg}");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
