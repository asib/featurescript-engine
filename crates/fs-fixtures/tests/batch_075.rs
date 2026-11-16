//! Builds fixture batch 075 (lofts with valid connections, guides and paths) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

// Circles lofted along a straight path (c006) through three sections: each a cubic through its circle every 45°, a
// natural spline along the loft (batch 112). Along a quarter circle (c007) both fail (`LOFT_FAILED`, as for batch
// 074 c006).

/// The lofts' faces come in an order of Onshape's own (c005: the bent side sewn in last).
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004", "c005"];

#[test]
fn batch_075_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-075");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (8, 8), "{report}");
    });
}
