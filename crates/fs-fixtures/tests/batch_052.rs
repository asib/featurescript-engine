//! Builds fixture batch 052 (conic and asymmetric fillets: sizes, angles, concave edges, sides) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

/// The fillets' B-spline sections are Onshape's (the same knots, through the same conic points: the STEP files
/// match), but Onshape's own volumes, areas and boxes of them stray from them and from the exact conics. A 2 × 5 mm
/// section measures 2.143786 mm² in Onshape along a convex edge (batch 003 c014 and c011 here agree), 2.144238
/// along a concave one (c015), against 2.147538 for the exported cubic and 2.146018 for the true quarter ellipse:
/// along c011's 30 mm edge, 0.11 mm³. c013's box reaches 2.7173 mm from the corner, its exported B-spline 2.7222
/// and the exact conic 2.7185. So their probes are compared to 0.15. c001's section is a parabola (rho 0.5), which
/// a cubic is exactly; c014 fails in both; c017's equal radii make a cylinder.
const LOOSE: &[(&str, f64)] = &[
    ("c002", 0.15),
    ("c003", 0.15),
    ("c004", 0.15),
    ("c005", 0.15),
    ("c006", 0.15),
    ("c007", 0.15),
    ("c008", 0.15),
    ("c009", 0.15),
    ("c010", 0.15),
    ("c011", 0.15),
    ("c012", 0.15),
    ("c013", 0.15),
    ("c015", 0.15),
    ("c016", 0.15),
    ("c018", 0.15),
    ("c019", 0.15),
];

#[test]
fn batch_052_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-052");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (20, 20), "{report}");
    });
}
