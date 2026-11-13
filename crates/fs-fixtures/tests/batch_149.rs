//! Builds fixture batch 149 (everyday sheet metal, a second round: a lipped tray, a Z bracket with holes, a bent tab,
//! a rolled hem, a tear-drop hem) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_149_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-149");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (6, 6), "{report}");
    });
}
