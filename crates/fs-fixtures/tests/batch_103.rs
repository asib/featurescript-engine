//! Builds fixture batch 103 (a Part Studio drawn in Onshape's UI, as Onshape writes it: its sketch by initial guess, its compressed queries, its tab imported and derived) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_103_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-103");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], &[]).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 9), "{report}");
    });
}
