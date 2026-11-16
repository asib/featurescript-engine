//! Builds fixture batch 112 (Onshape's guided and pathed lofts, sphere extensions and a rolled wall's hole, probed
//! for their poles) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine and Onshape don't agree on (see docs/std-coverage.md): a sphere's band extended round it (c007,
/// c008: Onshape's new edge depends on the whole band); a hole cut across a rolled wall (c009: Onshape's wall is its
/// own adaptive fit, with two planar slivers). Circles lofted along a straight path through sections (c004–c006)
/// match on their bodies: each section a cubic through its circle every 45°, a natural spline along the loft.
const KNOWN_GAPS: &[&str] = &["c007", "c008", "c009"];

/// The probes return Onshape's poles, its own fits, which the engine's match for lofts with a guide crossing a side
/// (c001–c003: rows through 8, 4 and 2 spans along the arc, batch 174); elsewhere only the bodies are compared.
const BODIES_ONLY: &[&str] = &["c004", "c005", "c006", "c007", "c008", "c009"];

/// The guided lofts' faces come in an order of Onshape's own (the bent side sewn in last).
const UNORDERED: &[&str] = &["c001", "c002", "c003"];

#[test]
fn batch_112_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-112");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], BODIES_ONLY, UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (10 - KNOWN_GAPS.len(), 10), "{report}");
    });
}
