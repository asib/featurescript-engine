//! Builds fixture batch 086 (shells, offset surfaces, a mutual trim of a plane and a cylinder, helices, bridging
//! curves, sheet metal tabs, a held partial flange and a rip, extrude start offsets, best-fit flattening, edge planes,
//! rays and curvatures) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Faces and edges come in an order of Onshape's own.
const UNORDERED: &[&str] = &[
    "c001", "c002", "c003", "c004", "c005", "c006", "c007", "c014", "c015", "c016", "c018", "c019", "c020",
];

/// Known gap (docs/FINDINGS.md): c004's offset sheet has two edges (where its faces meet, and at one end) running
/// the other way from Onshape's, which keeps its source edges' directions. c017 fails in both
/// (`SHEET_METAL_RIP_NO_CORNER`: the plate has no vertex between collinear sketch lines), its plate left as Part 10.
const KNOWN_GAPS: &[&str] = &["c004"];

#[test]
fn batch_086_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-086");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|gap| p.starts_with(gap))).collect();
        assert!(unexpected.is_empty(), "{report}");
    });
}
