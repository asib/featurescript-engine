//! Compares a batch's probe lines in full (faces in any order) for the cases given, printing which match Onshape's:
//! for finding cases a test compares only on their bodies that now match in full.
//!
//! Usage: cargo run --release -p fs-fixtures --example full_probes -- fixtures/batch-058 c001,c002 [tolerance]
//!
//! With a tolerance (mm), the cases' probe values are compared to within it instead of 1e-3.

use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let batch = args.next().expect("a batch directory");
    let cases: Vec<String> = args.next().unwrap_or_default().split(',').filter(|c| !c.is_empty()).map(String::from).collect();
    let tolerance: Option<f64> = args.next().and_then(|t| t.parse().ok());
    let code = fs_interp::run_with_large_stack(move || {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(&batch);
        let unordered: Vec<&str> = cases.iter().map(String::as_str).collect();
        let loose: Vec<(&str, f64)> = match tolerance {
            Some(t) => unordered.iter().map(|&c| (c, t)).collect(),
            None => Vec::new(),
        };
        match fs_fixtures::check_batch_unordered(&dir, None, &loose, &[], &unordered) {
            Ok(report) => {
                let problems = report.problems();
                for case in &cases {
                    let mine: Vec<&String> = problems.iter().filter(|p| p.starts_with(case.as_str())).collect();
                    match mine.first() {
                        None => println!("{case} matches"),
                        Some(p) => println!("{case} differs: {}", p.chars().take(140).collect::<String>()),
                    }
                }
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
