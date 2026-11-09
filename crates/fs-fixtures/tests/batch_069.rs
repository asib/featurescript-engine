//! Builds fixture batch 069 (a rolled wall with a flange beyond it, unfolded and refolded) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

/// The parts' faces come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004"];

#[test]
fn batch_069_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-069");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (5, 5), "{report}");
    });
}
