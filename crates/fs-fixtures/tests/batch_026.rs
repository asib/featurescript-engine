//! Builds fixture batch 026 (reapplied feature patterns, move face creating surfaces, hole tips, clearance and tapped holes) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_026_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-026");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (7, 7), "{report}");
    });
}
