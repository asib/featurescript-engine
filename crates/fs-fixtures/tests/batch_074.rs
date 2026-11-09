//! Builds fixture batch 074 (lofts with connections, guides and a path) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Circles lofted along a quarter circle (c006): Onshape fails (`LOFT_FAILED`) for a reason not yet known; the
/// engine ignores the path and lofts straight between them.
const KNOWN_GAPS: &[&str] = &["c006", "failed probe"];
/// The loft's faces come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c001"];

#[test]
fn batch_074_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-074");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        let problems: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|g| p.starts_with(g))).collect();
        assert!(problems.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (5, 7), "{report}");
    });
}
