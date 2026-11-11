//! Builds fixture batch 100 (std's API builtins no std feature calls: flipping a sheet, a B-spline surface, evaluating a spline, parsing JSON, mass properties, deviations, tolerances, a curvature derivative, a B-spline approximation, debug calls) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_100_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-100");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], &[]).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (11, 11), "{report}");
    });
}
