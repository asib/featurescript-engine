//! Builds fixture batch 001 on the engine, as Onshape built it, and compares every part with Onshape's STEP.

use std::path::PathBuf;

#[test]
fn batch_001_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-001");
        let step = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("batch-001-engine.step");
        let report = fs_fixtures::check_batch(&batch, Some(&step)).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.parts.len(), 100, "{report}");
        // The six cases Onshape couldn't build fail on the engine too, so the generator skips them.
        assert!(
            report.output.contains("batch-001 failed cases: [ c065 , c069 , c097 , c102 , c105 , c106 ]"),
            "{}",
            report.output
        );
    });
}
