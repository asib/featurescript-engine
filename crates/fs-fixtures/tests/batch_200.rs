//! Builds fixture batch 200 (frames and sweeps locked to cones and tori) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine doesn't build as Onshape does yet (see docs/FINDINGS.md): a frame along a cone's ruling, locked
/// to the cone (c003), whose faces along the ruling Onshape makes of another kind than planes; a frame and a sweep
/// along a circle on a torus, locked to it (c004, c005), their faces 0.25 and 0.09 mm² off Onshape's.
const KNOWN_GAPS: &[&str] = &["c003", "c004", "c005"];

#[test]
fn batch_200_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-200");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case) || p.contains(&format!("{case}."))))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
    });
}
