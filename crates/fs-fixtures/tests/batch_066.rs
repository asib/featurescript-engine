//! Builds fixture batch 066 (a second survey: isocline splits and geodesic offsets on B-spline faces, sheet and
//! curve extensions, a sweep along a line and a spline, a ruled surface off a B-spline face) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): a lofted sheet extended past a
/// straight side, which Onshape carries on along the arcs (c002).
const KNOWN_GAPS: &[&str] = &["c002"];

/// The split loft: its pieces' areas within 0.002 mm² of Onshape's (c001). The offset across the loft's side: the
/// engine walks geodesics in steps, within 0.0021 mm of Onshape's curve (c004).
const LOOSE: &[(&str, f64)] = &[("c001", 0.003), ("c004", 0.003)];

/// The ruled sheet's edges run as the engine's loft edge does, the other way from Onshape's: only its body (plane,
/// area, bounds) is compared (c006).
const BODIES_ONLY: &[&str] = &["c006"];

/// The split loft's faces come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c001"];

#[test]
fn batch_066_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-066");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, BODIES_ONLY, UNORDERED).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case)) && !p.starts_with("failed probe"))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (7 - KNOWN_GAPS.len() - 1, 7), "{report}");
    });
}
