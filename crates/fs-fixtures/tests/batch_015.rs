//! Builds fixture batch 015 (sheet metal corner breaks, unfold and refold, partial flanges) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_015_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-015");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (8, 8), "{report}");
    });
}
