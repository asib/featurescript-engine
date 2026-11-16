//! Builds fixture batch 181 (lofts between polygons of different corner counts, and polygons connected to circles) on
//! the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine doesn't build yet (see docs/FINDINGS.md): a square connected to a circle at two corners
/// (c007, c008: Onshape's sides take eight spans round each quarter, its circle exact). The engine refuses them, which
/// the failed cases' line shows.
const KNOWN_GAPS: &[&str] = &["c007", "c008", "failed probe"];

/// A rectangle connected to a circle once (c009): its sides Onshape's pole for pole, Onshape's circular end measured on
/// other approximations of the arcs, as in batch 180.
const LOOSE: &[(&str, f64)] = &[("c009", 0.1)];

/// Onshape lists the faces in its own order.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008", "c009"];

#[test]
fn batch_181_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-181");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (7, 10), "{report}");
    });
}
