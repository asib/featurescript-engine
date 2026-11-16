//! Builds fixture batch 180 (lofts with connections between profiles other than two polygons) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine doesn't build as Onshape does yet: none. The triangle connected to a square (c007) matches: the
/// triangle's edge opposite the connected corner split at its middle (batch 181).
const KNOWN_GAPS: &[&str] = &[];

/// A polygon connected to a circle once: the engine's sides are Onshape's pole for pole (within 1.7e-4 mm), but
/// Onshape's circular end is bounded by other approximations of the arcs than its sides' (314.1631 mm² where theirs
/// bound 314.1386), 0.025 mm² of area and up to 0.09 mm³ of volume apart; twice (c003), with twice as many spans,
/// within 0.002.
const LOOSE: &[(&str, f64)] =
    &[("c001", 0.1), ("c002", 0.1), ("c003", 0.002), ("c004", 0.1), ("c005", 0.1), ("c006", 0.1)];

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
        assert_eq!(report.probes_matched, (8, 8), "{report}");
    });
}
