//! Builds fixture batch 034 (locked sweeps, sweeps round sharp corners, routing through vertices, frame overrides,
//! tagged frame profiles, cone isoclines, ANSI threads) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_034_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-034");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 9), "{report}");
    });
}
