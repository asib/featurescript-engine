//! Runs a batch's generator on the engine and prints what it printed: the probe lines and the failed cases.
//!
//! Usage: cargo run --release -p fs-fixtures --example run_batch -- fixtures/batch-002

use std::path::PathBuf;

fn main() {
    let batch = std::env::args().nth(1).unwrap_or_else(|| "fixtures/batch-002".into());
    let code = fs_interp::run_with_large_stack(move || {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(&batch);
        match fs_fixtures::build(&dir) {
            Ok(built) => {
                print!("{}", built.output);
                eprintln!("built {batch} in {:.0} ms: {} named solids", built.seconds * 1000.0, built.parts.len());
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
