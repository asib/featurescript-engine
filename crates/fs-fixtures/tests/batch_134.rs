//! Builds fixture batch 134 (everyday parts: a flange, a ribbed bracket, a keyed shaft, a slotted plate, a pocket, a
//! cup) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_134_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-134");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (7, 7), "{report}");
    });
}
