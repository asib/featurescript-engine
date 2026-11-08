//! Builds fixture batch 017 (sheet metal cuts, holes, patterns, blends, make joint, bend relief, PCB, loft) on the
//! engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_017_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-017");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (15, 15), "{report}");
    });
}
