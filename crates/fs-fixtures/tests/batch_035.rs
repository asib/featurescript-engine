//! Builds fixture batch 035 (routing splines with inner derivatives, routing points on curves, realigned decals,
//! cut lists of merged frames) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_035_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-035");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (15, 15), "{report}");
    });
}
