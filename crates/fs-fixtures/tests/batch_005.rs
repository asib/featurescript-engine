//! Builds fixture batch 005 (curve features, surface features and mate connectors) on the engine and compares it
//! with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/std-coverage.md): a boundary surface's parameterization (c019).
const KNOWN_GAPS: &[&str] = &["c019"];

#[test]
fn batch_005_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-005");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (30 - KNOWN_GAPS.len(), 30), "{report}");
    });
}
