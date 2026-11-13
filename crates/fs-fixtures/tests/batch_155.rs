//! Builds fixture batch 155 (probes: which of two tangent lines holds) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/FINDINGS.md): one line drawn further off than the other (c005, c006). Onshape still
/// keeps the line starting first, but moves the other along its normal as it turns it; the engine moves it only across
/// its main axis.
const KNOWN_GAPS: &[&str] = &["c005", "c006"];

#[test]
fn batch_155_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-155");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (6, 8), "{report}");
    });
}
