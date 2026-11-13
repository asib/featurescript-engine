//! Builds fixture batch 150 (probes: a split cylinder's edges, under-constrained sketches settling) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/FINDINGS.md): two circles with their radii dimensioned and two lines drawn a little
/// off held tangent to both (c008): Onshape keeps the second line where it was and turns the first about its middle,
/// moving both circles a little; the engine keeps both lines and moves the circles.
const KNOWN_GAPS: &[&str] = &["c008"];

#[test]
fn batch_150_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-150");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (8, 9), "{report}");
    });
}
