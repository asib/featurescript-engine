//! Builds fixture batch 163 (probes of the extra term in a boundary surface's derivative at magnitudes other than 1, by the knots
//! across its profiles) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/FINDINGS.md): to a straight spline of three uneven spans (c005), where Onshape's
/// magnitude fit isn't the one found for even knots.
const KNOWN_GAPS: &[&str] = &["c005"];

#[test]
fn batch_163_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-163");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 10), "{report}");
    });
}
