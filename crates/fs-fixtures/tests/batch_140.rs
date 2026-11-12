//! Builds fixture batch 140 (everyday parts: a hinge, a gear blank, a sheet metal box, a pipe elbow, a bottle, a
//! handle, counterbored holes, a drafted block) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_140_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-140");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 9), "{report}");
    });
}
