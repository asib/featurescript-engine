//! Builds fixture batch 029 (sheet metal tear drop hems) on the engine and compares it with Onshape's, the engine's
//! own STEP of it read back too (each hemmed plate's edges where the hem's wall meets its bend once read back 2e-6
//! off in volume, its lines 3e-7 off the faces; they now lie on them).

use std::path::PathBuf;

#[test]
fn batch_029_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-029");
        let step = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("batch-029-engine.step");
        let report = fs_fixtures::check_batch(&batch, Some(&step)).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (4, 4), "{report}");
    });
}
