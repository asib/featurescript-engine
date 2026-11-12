//! Builds fixture batch 139 (std's last features, ruled surface overrides on a curved edge, everyday parts) on the
//! engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_139_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-139");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 9), "{report}");
    });
}
