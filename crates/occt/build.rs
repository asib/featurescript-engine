//! Builds the C++ half of the bridge against the system's OCCT 7.9 (Ubuntu's `libocct-*-dev` packages).
//!
//! `OCCT_INCLUDE_DIR` and `OCCT_LIB_DIR` override where the headers and libraries are. For WebAssembly
//! (`wasm32-unknown-emscripten`), they name an Emscripten build of OCCT's static libraries (see crates/fs-wasm),
//! which link statically with the visualization toolkits OCCT's XCAF needs, and C++ exceptions are WebAssembly's.

use std::env;
use std::path::PathBuf;

const LIBRARIES: &[&str] = &[
    "TKernel",
    "TKMath",
    "TKG2d",
    "TKG3d",
    "TKGeomBase",
    "TKGeomAlgo",
    "TKBRep",
    "TKTopAlgo",
    "TKPrim",
    "TKBO",
    "TKBool",
    "TKFillet",
    "TKOffset",
    "TKFeat",
    "TKShHealing",
    "TKXSBase",
    "TKDE",
    "TKDESTEP",
    "TKCDF",
    "TKLCAF",
    "TKCAF",
    "TKXCAF",
    "TKMesh",
    "TKHLR",
];

/// What OCCT's XCAF needs besides, linked statically for WebAssembly.
const WASM_EXTRA: &[&str] = &["TKVCAF", "TKV3d", "TKService"];

fn main() {
    let include = PathBuf::from(env::var("OCCT_INCLUDE_DIR").unwrap_or_else(|_| "/usr/include/opencascade".into()));
    let lib = env::var("OCCT_LIB_DIR").unwrap_or_else(|_| "/usr/lib/x86_64-linux-gnu".into());
    assert!(
        include.join("Standard.hxx").exists(),
        "OCCT headers not found in {}; install libocct-*-dev or set OCCT_INCLUDE_DIR",
        include.display()
    );

    let wasm = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("emscripten");
    let mut build = cxx_build::bridge("src/lib.rs");
    if wasm {
        build.flag("-fwasm-exceptions");
    }
    build
        .file("cpp/occt.cc")
        .file("cpp/modeling.cc")
        .file("cpp/measure.cc")
        .file("cpp/curves.cc")
        .file("cpp/surfaces.cc")
        .std("c++17")
        // OCCT's headers aren't warning-clean; -isystem keeps their warnings out of ours.
        .flag(format!("-isystem{}", include.display()))
        .flag_if_supported("-Wno-deprecated-declarations")
        .compile("occt-bridge");

    println!("cargo:rustc-link-search=native={lib}");
    if wasm {
        // Static archives, linked only into the final module (wasm-ld finds symbols in any order).
        let mut names: Vec<&str> = LIBRARIES.iter().chain(WASM_EXTRA).copied().collect();
        names.sort_unstable();
        names.dedup();
        for name in names {
            println!("cargo:rustc-link-lib=static:-bundle={name}");
        }
    } else {
        for name in LIBRARIES {
            println!("cargo:rustc-link-lib=dylib={name}");
        }
    }
    println!("cargo:rerun-if-changed=cpp");
    println!("cargo:rerun-if-env-changed=OCCT_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=OCCT_LIB_DIR");
}
