//! Builds fixture batch 045 (composite parts with sheet metal derived) on the engine
//! and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_045_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-045");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (4, 4), "{report}");
    });
}
