//! Builds fixture batch 146 (everyday parts, a seventh round: an enclosure, a pipe elbow, a flanged pipe, a split
//! lid, a drafted box, a mirrored pocket, a gear-like disc, a keyed shaft) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

#[test]
fn batch_146_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-146");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 9), "{report}");
    });
}
