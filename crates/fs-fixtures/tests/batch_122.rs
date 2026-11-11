//! Builds fixture batch 122 (everyday variants: chamfers, shells, drafts, faces deleted, enclosures, thickened and split) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_122_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-122");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (21, 21), "{report}");
    });
}
