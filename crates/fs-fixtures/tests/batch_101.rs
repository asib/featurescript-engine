//! Builds fixture batch 101 (curvature derivatives on a cylinder, a torus, a sphere and a cone, internal tolerances and faults, a cylinder approximated as a B-spline surface) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_101_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-101");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], &[]).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 9), "{report}");
    });
}
