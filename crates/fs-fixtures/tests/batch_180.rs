//! Builds fixture batch 180 (lofts with connections between profiles other than two polygons) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine doesn't build yet (see docs/FINDINGS.md): a square connected to a circle at two corners at once
/// (c003: Onshape's sides take eight spans round each quarter, its circle exact), and a triangle connected to a square
/// (c007: one side a planar fan from a corner to an edge). The engine refuses them, which the failed cases' line
/// shows.
const KNOWN_GAPS: &[&str] = &["c003", "c007", "failed probe"];

/// A polygon connected to a circle once: the engine's sides are Onshape's pole for pole (within 1.7e-4 mm), but
/// Onshape's circular end is bounded by other approximations of the arcs than its sides' (314.1631 mm² where theirs
/// bound 314.1386), 0.025 mm² of area and up to 0.09 mm³ of volume apart.
const LOOSE: &[(&str, f64)] = &[("c001", 0.1), ("c002", 0.1), ("c004", 0.1), ("c005", 0.1), ("c006", 0.1)];

/// Onshape lists the faces in its own order.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006", "c007"];

#[test]
fn batch_180_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-180");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (5, 8), "{report}");
    });
}
