//! Builds fixture batch 150 (probes: a split cylinder's edges, under-constrained sketches settling) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_150_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-150");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 9), "{report}");
    });
}
