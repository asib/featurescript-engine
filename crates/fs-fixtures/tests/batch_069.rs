//! Builds fixture batch 069 (a rolled wall with a flange beyond it, unfolded and refolded) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): unfolding a rolled wall with a
/// bend beyond it, and refolding it (c002–c004: the engine finds no hold entity). Onshape's results are kept for
/// that work.
const KNOWN_GAPS: &[&str] = &["c002", "c003", "c004", "failed probe"];

/// The part's faces come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c001"];

#[test]
fn batch_069_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-069");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (1, 5), "{report}");
    });
}
