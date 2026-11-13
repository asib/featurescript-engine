//! Builds fixture batch 161 (probes: the poles of Onshape's three-sided and tangent fills and of boundary surfaces at magnitude 2;
//! fills whose curvature condition or guide the plane meets) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/FINDINGS.md): three-sided fills (c001, c002) and fills tangent to sheets they don't
/// continue (c003, c004), Onshape's own adaptive fits (bicubic, knots refined unevenly over a plane in metres; a thin
/// plate is 0.11 to 0.16 mm off them). The engine refuses the tangent ones, which the failed cases' line shows.
const KNOWN_GAPS: &[&str] = &["c001", "c002", "c003", "c004", "failed probe"];

#[test]
fn batch_161_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-161");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (5, 10), "{report}");
    });
}
