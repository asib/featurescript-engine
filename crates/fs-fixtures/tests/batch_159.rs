//! Builds fixture batch 159 (boundary surfaces matching the tangency of flat faces next to their profiles; which way flat fills face; which way an
//! extruded sheet's edges run) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_159_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-159");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (15, 15), "{report}");
    });
}
