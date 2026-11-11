//! Builds fixture batch 126 (faces moved and healed on curved faces, feature patterns, a merged mirror, drafts, a
//! partial revolve) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_126_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-126");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (11, 11), "{report}");
    });
}
