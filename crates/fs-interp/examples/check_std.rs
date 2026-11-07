//! Loads std through geometry.fs, evaluates every top-level constant, and reports failures, missing builtins
//! and timings. Run with `cargo run --release -p fs-interp --example check_std`.

use std::time::Instant;

fn main() {
    fs_interp::run_with_large_stack(|| {
        let start = Instant::now();
        let mut engine = fs_interp::Engine::with_vendored_std();
        engine.load_geometry().expect("load geometry.fs");
        let loaded = start.elapsed();
        let modules = engine.interp.modules.list.len();
        let start = Instant::now();
        let failures = engine.check_all_constants();
        let evaluated = start.elapsed();
        for failure in &failures {
            println!("FAIL {failure}");
        }
        println!("missing builtins: {:?}", engine.interp.missing_builtins);
        println!(
            "{modules} modules loaded in {:.0} ms; constants evaluated in {:.0} ms; {} failures",
            loaded.as_secs_f64() * 1000.0,
            evaluated.as_secs_f64() * 1000.0,
            failures.len()
        );
    });
}
