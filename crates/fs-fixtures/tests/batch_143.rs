//! Builds fixture batch 143 (everyday sheet metal: patterned holes on walls, a hole grid, a slotted hemmed flange, a
//! jog with holes, rounded corners) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_143_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-143");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (7, 7), "{report}");
    });
}
