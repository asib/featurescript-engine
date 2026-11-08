//! Builds fixture batch 010 (full round, variable and partial fillets, splines through edges, bridging curves,
//! wraps, face blends and moving faces) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_010_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-010");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (11, 11), "{report}");
    });
}
