//! Runs every case function of a fixture batch on a fresh context and summarises how each one ends.
//!
//! Usage: `cargo run --release -p fs-geom --example run_cases [fixtures/batch-001] [--lenient] [--only c001,c002]`
//!
//! With `--lenient`, builtins that aren't written yet return `undefined` instead of throwing, which shows more of
//! the builtins a run would reach. The outcomes are meaningless then; only the list of builtins is useful.

use fs_interp::{Value, run_with_large_stack};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let lenient = args.iter().any(|a| a == "--lenient");
    let verbose = args.iter().any(|a| a == "--verbose");
    let only: Option<Vec<String>> = args
        .iter()
        .position(|a| a == "--only")
        .and_then(|i| args.get(i + 1))
        .map(|list| list.split(',').map(String::from).collect());
    let batch = args
        .iter()
        .enumerate()
        .find(|(i, a)| !a.starts_with("--") && (*i == 0 || args[i - 1] != "--only"))
        .map(|(_, a)| a.clone())
        .unwrap_or_else(|| "fixtures/batch-001".into());
    run_with_large_stack(move || {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut engine = fs_geom::engine();
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
            .filter(|name| only.as_ref().is_none_or(|only| only.contains(name)))
            .collect();

        let mut outcomes: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let start = Instant::now();
        for name in &names {
            let mut solids = String::new();
            let outcome = engine.eval(module, "newContextWithDefaults()").and_then(|context| {
                let id = engine.eval(module, &format!("makeId(\"{name}\")"))?;
                let result = engine.call(module, name, vec![context.clone(), id]);
                solids = fs_geom::with_model(&context, describe_solids).unwrap_or_default();
                result
            });
            let key = match outcome {
                Ok(_) => "completed".to_string(),
                Err(e) => describe(&e.0),
            };
            if verbose {
                println!("{name}: {key}{solids}");
            }
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

/// Each solid body's volume (mm³) and face, edge and vertex counts.
fn describe_solids(model: &fs_geom::model::Model) -> String {
    let mut out = String::new();
    for body in model.bodies.iter().filter(|b| b.body_type == fs_geom::model::BodyType::Solid) {
        let volume = body.shape.volume().unwrap_or(f64::NAN) * 1e9;
        let valid = if body.shape.is_valid() { "" } else { " INVALID" };
        out += &format!(
            "  [{volume:.3} mm3, {}f {}e {}v{valid}]",
            body.faces.len(),
            body.edges.len(),
            body.vertices.len()
        );
    }
    out
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
