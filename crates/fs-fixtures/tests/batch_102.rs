//! Builds fixture batch 102 (std's internal API functions called from a feature: a feature's name, a parameter's tolerance info, a compressed query, booleaned patterns) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_102_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-102");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], &[]).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (6, 6), "{report}");
    });
}
