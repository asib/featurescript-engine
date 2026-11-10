//! Builds fixture batch 094 (uploaded files: an image's and a STEP file's BLOB_DATA, the image in sketches, the
//! STEP file through importForeign) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_094_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-094");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], &[]).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (10, 10), "{report}");
    });
}
