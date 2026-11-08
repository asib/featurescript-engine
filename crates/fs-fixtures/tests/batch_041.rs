//! Builds fixture batch 041 (section parts for assemblies: transformed parts, several planes) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_041_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-041");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (5, 5), "{report}");
    });
}
