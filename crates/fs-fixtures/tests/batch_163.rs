//! Builds fixture batch 163 (probes of the extra term in a boundary surface's derivative at magnitudes other than 1, by the knots
//! across its profiles) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_163_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-163");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (10, 10), "{report}");
    });
}
