//! Builds fixture batch 080 (face blend conic and inverse hold lines, a circle lofted to a square with a connection,
//! a rolled wall cut and added to, a quintic extended, a revolved sheet extended each way) on the engine and compares
//! it with Onshape's.

use std::path::PathBuf;

/// The revolved sheet extended past its top: Onshape's new top edge is its own fit, bulging 0.006 mm (c007, to 1e-4
/// of the area); extended round its side: to 8e-6 (c008).
const LOOSE: &[(&str, f64)] = &[("c007", 0.1), ("c008", 0.05)];

/// Faces and curved edges come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c001", "c005", "c007", "c008"];

/// Known gaps (docs/std-coverage.md): Onshape refuses an inverse tangent hold line on the edge of a side
/// (`FACE_BLEND_DIMENSION_TOO_LARGE`, c002) for a reason one case doesn't show; a hole cut across a rolled wall
/// (c004, to 3.8e-5 of the volume) has its wall in two B-spline faces and two planar slivers in Onshape, four
/// quarters in the engine.
const KNOWN_GAPS: &[&str] = &["c002", "c004", "failed probe"];

#[test]
fn batch_080_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-080");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|gap| p.starts_with(gap))).collect();
        assert!(unexpected.is_empty(), "{report}");
    });
}
