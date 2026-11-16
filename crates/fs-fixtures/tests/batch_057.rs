//! Builds fixture batch 057 (lofts to points and matching faces, sweeps twisted along a line and an arc, a torus
//! sheet extended, tilted torus isoclines) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// A loft from a square to a point, normal to the square: Onshape's sides are fits that bulge between the corners
/// and the middles of the edges by up to 0.003 mm (0.28 mm³ of volume). Lofts from a block's top matching its
/// sides: Onshape's sides, a cubic along each edge, bulge between the corners (0.41 mm³, 7e-5 of the volume; 0.2
/// at magnitude 2). Sweeps twisted or scaled along a line and an arc: as along an arc alone (batch 048), Onshape's
/// surfaces are approximations, within 7e-5 of the volume. Matching the blocks' curvature (c007): Onshape's quintic sides are the tangency-matched cubics raised to degree
/// 5 with the second row moved for the curvature, exactly at the corners, but bulge between them by a rule not
/// found (its second row 0.015 mm higher mid-edge, and only half as much at twice the magnitude in c006): 2.05 mm³,
/// 3.6e-4 of the volume.
const LOOSE: &[(&str, f64)] =
    &[("c004", 0.3), ("c005", 0.5), ("c006", 0.3), ("c007", 2.1), ("c009", 0.5), ("c010", 0.5)];

/// Every case returns its faces' surfaces or points along its edges, to study. These are compared on their bodies
/// only; the rest match in full, in any order (the loft between circles, c008, periodic round them as Onshape's
/// sides are, among them). Through a circle between two points (c003) Onshape's seam is half a turn round from the
/// engine's.
const CASES: &[&str] = &["c003", "c004", "c005", "c006", "c007", "c009", "c010"];

/// Compared in full, faces in any order.
const UNORDERED: &[&str] = &["c001", "c002", "c008", "c011", "c012", "c013"];

#[test]
fn batch_057_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-057");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, CASES, UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (14, 14), "{report}");
    });
}
