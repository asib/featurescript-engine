//! Builds fixture batch 007 (surface coordinate systems, selection queries, and more options of hole, rib, body
//! draft, revolve, split face, transform, isocline, name entity and query variable) on the engine and compares it
//! with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_007_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-007");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (28, 28), "{report}");
    });
}
