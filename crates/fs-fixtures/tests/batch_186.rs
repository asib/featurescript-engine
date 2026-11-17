//! Builds fixture batch 186 (lofts with connections checking batches 180–182's rules on other cases; sketch text in
//! Roboto Slab Bold and other letters in the fonts batch 185 matched) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// A square connected to a circle at every corner (c004): Onshape runs its sides clockwise round the loft, the other
/// way from every other connected loft (their surfaces otherwise the engine's, areas within 3e-5 mm²), so only the
/// bodies are compared (docs/FINDINGS.md).
const BODIES_ONLY: &[&str] = &["c004"];

/// Onshape's circular ends measured on other approximations of the arcs than its sides' edges (as in batch 180):
/// volumes up to 0.02 mm³ off.
const LOOSE: &[(&str, f64)] =
    &[("c001", 0.01), ("c002", 0.002), ("c003", 0.02), ("c004", 0.01), ("c005", 0.002), ("c006", 0.002)];

/// Onshape lists the faces and regions in its own order.
const UNORDERED: &[&str] = &[
    "c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008", "c009", "c010", "c011", "c012", "c013", "c014",
    "c015", "c016",
];

#[test]
fn batch_186_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-186");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, BODIES_ONLY, UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (17, 17), "{report}");
    });
}
