//! Runs every case function of a fixture batch on a fresh context and summarises how each one ends.
//!
//! Usage: `cargo run --release -p fs-interp --example run_cases [fixtures/batch-001] [--lenient]`
//!
//! With `--lenient`, builtins that aren't written yet return `undefined` instead of throwing, which shows more of
//! the builtins a run would reach. The outcomes are meaningless then; only the list of builtins is useful.

use fs_interp::{Engine, Value, run_with_large_stack};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let lenient = args.iter().any(|a| a == "--lenient");
    let batch = args.into_iter().find(|a| !a.starts_with("--")).unwrap_or_else(|| "fixtures/batch-001".into());
    run_with_large_stack(move || {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut engine = Engine::with_vendored_std();
        engine.interp.lenient_builtins = lenient;
        let start = Instant::now();
        let module = engine.load_file(root.join(&batch).join("cases.fs")).expect("load cases.fs");
        println!("loaded {batch} and std in {:.0} ms", start.elapsed().as_secs_f64() * 1000.0);

        let names: Vec<String> = engine.interp.modules.list[module]
            .ast
            .items
            .iter()
            .filter_map(|item| match &item.kind {
                fs_lang::ast::ItemKind::Function { name, .. } => Some(name.name.clone()),
                _ => None,
            })
            .collect();

        let mut outcomes: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let start = Instant::now();
        for name in &names {
            let outcome = engine.eval(module, "newContextWithDefaults()").and_then(|context| {
                let id = engine.eval(module, &format!("makeId(\"{name}\")"))?;
                engine.call(module, name, vec![context, id])
            });
            let key = match outcome {
                Ok(_) => "completed".to_string(),
                Err(e) => describe(&e.0),
            };
            outcomes.entry(key).or_default().push(name.clone());
        }
        let elapsed = start.elapsed();
        for (outcome, cases) in &outcomes {
            println!(
                "{:>3} cases: {outcome}   ({}{})",
                cases.len(),
                cases[..cases.len().min(6)].join(" "),
                if cases.len() > 6 { " ..." } else { "" }
            );
        }
        println!("missing builtins hit: {:?}", engine.interp.missing_builtins);
        println!("{} cases in {:.0} ms", names.len(), elapsed.as_secs_f64() * 1000.0);
    });
}

/// A thrown string, or a library error map's message.
fn describe(exception: &Value) -> String {
    if let Some(s) = exception.as_str() {
        return s.to_string();
    }
    match exception.get("message") {
        Some(message) => format!("error {message:?}"),
        None => format!("{exception:?}"),
    }
}
