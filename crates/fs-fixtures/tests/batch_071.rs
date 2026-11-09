//! Builds fixture batch 071 (face blends limited by planes, capped, between a floor and a cylinder or a wall of
//! several faces; Béziers; half a tilted torus split; a flex PCB's one bend unfolded; a hole cut in the flat of a
//! rolled wall with a flange) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The hole Onshape wraps onto the rolled wall is its own fit of the curve: the part's area 0.016 mm² off (4e-6).
const LOOSE: &[(&str, f64)] = &[("c012", 0.03)];
/// Onshape's curve made of a sketch's edges keeps their order and directions there; the engine's runs along the
/// chain. The sketch's own edges (the Bézier and its control polygon) match.
const BODIES_ONLY: &[&str] = &["c008"];
/// Faces and bend lines come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c001", "c002", "c004", "c005", "c006", "c007", "c010", "c011", "c012"];

#[test]
fn batch_071_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-071");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, BODIES_ONLY, UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (13, 13), "{report}");
    });
}
