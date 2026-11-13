//! Builds fixture batch 165 (variable fillets with radii at points along the edge) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

#[test]
fn batch_165_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-165");
        let step = batch.join("onshape.step");
        let report = fs_fixtures::check_batch(&batch, Some(&step)).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (6, 6), "{report}");
    });
}
