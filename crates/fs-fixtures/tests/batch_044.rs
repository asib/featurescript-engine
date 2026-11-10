//! Builds fixture batch 044 (sheet metal lofts: L profiles, skew lines, unlike profiles, arcs, added to a plate) on
//! the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_044_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-044");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], &[]).unwrap();
        assert!(report.problems().is_empty(), "{report}");
    });
}
