//! Builds fixture batch 162 (probes of the extra term in a boundary surface's derivative at magnitudes other than 1) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/FINDINGS.md): to straight splines with uneven knots (c002, c003), where Onshape's
/// magnitude fit isn't the one found for even knots (its interior fraction 0.858 and 0.855 where that gives 0.843 and
/// 0.815).
const KNOWN_GAPS: &[&str] = &["c002", "c003"];

#[test]
fn batch_162_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-162");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (8, 10), "{report}");
    });
}
