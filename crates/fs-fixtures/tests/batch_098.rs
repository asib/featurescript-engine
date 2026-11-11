//! Builds fixture batch 098 (build functions from the Feature Studio built, instantiated, derived and mirrored) on
//! the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_098_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-098");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], &[]).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (7, 7), "{report}");
    });
}
