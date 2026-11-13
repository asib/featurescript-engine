//! Builds fixture batch 148 (everyday parts, a ninth round: a pillow block, an up-to-next column, a shelled dome, a
//! mirrored bracket, a pin grid, a thickened offset, a rib, a two-distance chamfer) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

#[test]
fn batch_148_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-148");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 9), "{report}");
    });
}
