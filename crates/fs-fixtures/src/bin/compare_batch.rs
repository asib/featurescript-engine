//! Builds a fixture batch on the engine, compares it with Onshape's STEP and writes the engine's model as STEP.
//!
//! Usage: `cargo run --release -p fs-fixtures --bin compare_batch [fixtures/batch-001]`
//!
//! The engine's model goes to `target/fixtures/<batch>/engine.step`, next to nothing else, so it can be opened
//! beside `onshape.step` when a part doesn't match.

use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let batch = std::env::args().nth(1).unwrap_or_else(|| "fixtures/batch-001".into());
    let code = fs_interp::run_with_large_stack(move || {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = root.join(&batch);
        let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let step = root.join("target/fixtures").join(name).join("engine.step");
        let start = Instant::now();
        match fs_fixtures::check_batch(&dir, Some(&step)) {
            Ok(report) => {
                print!("{}", report.output);
                println!(
                    "built {batch} in {:.0} ms; checked in {:.1} s",
                    report.build_seconds * 1000.0,
                    start.elapsed().as_secs_f64()
                );
                print!("{report}");
                println!("engine model: {}", step.display());
                i32::from(!report.problems().is_empty())
            }
            Err(e) => {
                eprintln!("{e}");
                2
            }
        }
    });
    std::process::exit(code);
}
