//! Builds fixture batch 138 (frames locked to curved faces along other paths) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): a square tube round the four edges a
/// rectangle splits a cylinder's side along (c005), whose segments std orients from those edges' ends and directions,
/// which the engine doesn't give as Onshape does for a split's edges (Onshape's arcs' profiles come out turned 14°).
const KNOWN_GAPS: &[&str] = &["c005"];

#[test]
fn batch_138_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-138");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (6 - KNOWN_GAPS.len(), 6), "{report}");
    });
}
