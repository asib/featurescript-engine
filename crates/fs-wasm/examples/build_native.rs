//! The native twin of test.mjs: builds a Feature Studio's feature with parameter values through the customizer
//! interface, natively, to compare with the WebAssembly module.
//!
//! Usage: cargo run --release -p fs-wasm --example build_native -- FILE.fs FEATURE OUT_STEM [VALUES]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, file, feature, stem, rest @ ..] = &args[..] else {
        eprintln!("usage: build_native FILE.fs FEATURE OUT_STEM [VALUES]");
        std::process::exit(2);
    };
    let values = rest.first().cloned().unwrap_or_else(|| "{}".to_string());
    let (file, feature, stem) = (file.clone(), feature.clone(), stem.clone());
    let std_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../std");
    let code = fs_interp::run_with_large_stack(move || {
        let source = std::fs::read_to_string(&file).expect("read the Feature Studio");
        match fs_wasm::build(std_dir, &file, &source, &feature, &values, &stem) {
            Ok(log) => {
                print!("{log}");
                0
            }
            Err(e) => {
                eprintln!("{e}");
                1
            }
        }
    });
    std::process::exit(code);
}
