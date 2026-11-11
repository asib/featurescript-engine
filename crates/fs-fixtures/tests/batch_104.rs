//! Builds fixture batch 104 (a second Part Studio drawn in Onshape's UI: a trimmed spline, and splines and a point made by Use) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_104_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-104");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], &[]).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (6, 6), "{report}");
    });
}
