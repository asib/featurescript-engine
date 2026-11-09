//! Builds fixture batch 004 (sketch curves, surfaces, extrude bounds, sweep, loft, helix and fit spline) on the
//! engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/std-coverage.md): a loft between unlike profiles (a square and a circle), whose
/// sides come from each kernel's own matching of the profiles (c039).
const KNOWN_GAPS: &[&str] = &["c039"];

#[test]
fn batch_004_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-004");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (46 - KNOWN_GAPS.len(), 46), "{report}");
    });
}
