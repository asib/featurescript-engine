//! Builds fixture batch 088 (edge directions of extrudes, a revolve, a fillet, a chamfer and a hole) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

/// Edges come in an order of Onshape's own (their directions are compared).
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008", "c009", "c010"];

#[test]
fn batch_088_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-088");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
    });
}
