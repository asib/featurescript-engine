//! Builds fixture batch 156 (probes: circles without radius dimensions settling against tangent lines) on the engine
//! and compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/FINDINGS.md): two lines held tangent to two circles, one or neither dimensioned
/// (c001–c004). Onshape changes an undimensioned circle's radius (the second's where neither is) and slides the other
/// line straight up rather than turning it, the circle's centre moving 0.1 at 30° (a split found no rule for); the
/// engine keeps the radii and turns the line.
const KNOWN_GAPS: &[&str] = &["c001", "c002", "c003", "c004"];

#[test]
fn batch_156_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-156");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (3, 7), "{report}");
    });
}
