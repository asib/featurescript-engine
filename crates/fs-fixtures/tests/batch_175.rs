//! Builds fixture batch 175 (plain lofts through three or four squares, for their knots at the middle profiles) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The plain loft through three or more polygons puts its middle profiles by a rule not yet found (docs/FINDINGS.md):
/// c001, c002, c007 and c008 differ by their middle knots.
const KNOWN_GAPS: &[&str] = &["c001", "c002", "c007", "c008"];

/// Onshape lists the faces in its own order.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008"];

#[test]
fn batch_175_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-175");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (5, 9), "{report}");
    });
}
