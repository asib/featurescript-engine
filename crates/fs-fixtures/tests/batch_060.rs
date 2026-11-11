//! Builds fixture batch 060 (derived mirror, intersection curves split at seams and where they touch an edge,
//! lofts tangent to rectangles, hexagons and squares, extruding in a bent flat) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Cases the engine and Onshape don't agree on (see docs/std-coverage.md): none. `derivedMirror` of a build function
/// from the Feature Studio fails in both, "Execution error" (c001): it reads its source's part names, and Onshape won't
/// read the properties of a context the studio's own code built while a feature runs (batch 098; a Part Studio tab's it
/// reads, batch 096). The bent flat cases (c009, c010) query the wrong feature id and fail in both, with the same
/// error; their brackets are left, unnamed: Part 12 and Part 13 in both models.
const KNOWN_GAPS: &[&str] = &[];

/// Intersection curves: Onshape's are nearly, not exactly, parametrised by length; their middles by parameter are
/// within 0.02 mm of the engine's (c002–c005).
const LOOSE: &[(&str, f64)] = &[("c002", 0.025), ("c003", 0.025), ("c004", 0.025), ("c005", 0.025)];

/// Onshape lists a loft's sides in an order of its own (its internal ids): their faces are compared in any order.
const UNORDERED: &[&str] = &["c006", "c007", "c008"];

#[test]
fn batch_060_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-060");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], UNORDERED).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case)) && !p.starts_with("failed probe"))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (11, 11), "{report}");
    });
}
