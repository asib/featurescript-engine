//! Builds a Feature Studio's feature with one set of values after another on one engine, and checks each build
//! against the same values built on an engine that has kept nothing: the STL, the STEP and the log must be the same.
//!
//! Usage: cargo run --release -p fs-wasm --example rebuilds -- FILE.fs FEATURE VALUES...
//!
//! Each VALUES is a JSON object of parameter values. Prints how many operations each build ran, how many it took
//! from what was kept, and how many measurements it was given from what was remembered.

use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, file, feature, values @ ..] = &args[..] else {
        eprintln!("usage: rebuilds FILE.fs FEATURE VALUES...");
        std::process::exit(2);
    };
    let (file, feature, values) = (file.clone(), feature.clone(), values.to_vec());
    let std_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../std");
    let code = fs_interp::run_with_large_stack(move || {
        let source = std::fs::read_to_string(&file).expect("read the Feature Studio");
        let dir = std::env::temp_dir();
        let output = |model: &fs_wasm::Model, stem: &str| {
            let step = dir.join(format!("{stem}.step"));
            model.write_step(&step.to_string_lossy()).expect("STEP");
            let text = std::fs::read_to_string(&step).expect("read STEP");
            let step: String = text.lines().filter(|l| !l.starts_with("FILE_NAME")).collect::<Vec<_>>().join("\n");
            (model.stl(0.05).expect("coarse"), model.stl(0.01).expect("fine"), step, model.volumes().expect("volumes"))
        };
        // Fresh builds first, each on an engine of its own, which keeps nothing from the others.
        let fresh: Vec<_> = values
            .iter()
            .map(|v| {
                // A different path loads a new engine.
                let model = fs_wasm::model(std_dir, &format!("{file}#{v}"), &source, &feature, v).expect("fresh build");
                (output(&model, "fresh"), model.output.clone())
            })
            .collect();
        let mut failed = 0;
        for (v, expected) in values.iter().zip(&fresh) {
            let started = Instant::now();
            let model = fs_wasm::model(std_dir, &file, &source, &feature, v).expect("build");
            let took = started.elapsed();
            let counts = fs_geom::reuse::counts();
            let same = (output(&model, "kept"), model.output.clone()) == *expected;
            failed += usize::from(!same);
            println!(
                "{:>6.0} ms  ran {:>2}, kept {:>2}, recalled {:>2}  {}  {v}",
                took.as_secs_f64() * 1e3,
                counts.ran,
                counts.kept,
                counts.recalled,
                if same { "same as fresh" } else { "DIFFERS FROM FRESH" }
            );
        }
        i32::from(failed > 0)
    });
    std::process::exit(code);
}
