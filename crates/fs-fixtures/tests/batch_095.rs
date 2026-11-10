//! Builds fixture batch 095 (how FeatureScript can import an uploaded STEP file: importForeign and opImportForeign
//! given its BLOB_DATA and its data id, their errors caught) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_095_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-095");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], &[]).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (6, 6), "{report}");
    });
}
