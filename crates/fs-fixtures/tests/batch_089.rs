//! Builds fixture batch 089 (revolves about a profile's own edge, circles lofted along a quarter circle) on the engine
//! and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_089_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-089");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], &["c002", "c009"]).unwrap();
        assert!(report.problems().is_empty(), "{report}");
    });
}
