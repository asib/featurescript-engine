//! Builds fixture batch 169 (lofts whose profiles are more than one face) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

#[test]
fn batch_169_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-169");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (4, 4), "{report}");
    });
}
