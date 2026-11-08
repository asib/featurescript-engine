//! Builds fixture batch 015 (sheet metal corner breaks, unfold and refold, partial flanges) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/std-coverage.md): partial flanges, whose bend reliefs the engine doesn't cut yet
/// (c006, c007).
const KNOWN_GAPS: &[&str] = &["c006", "c007"];

#[test]
fn batch_015_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-015");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (8 - KNOWN_GAPS.len(), 8), "{report}");
    });
}
