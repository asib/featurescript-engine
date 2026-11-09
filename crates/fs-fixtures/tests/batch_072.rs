//! Builds fixture batch 072 (cut lists of frames along an arc and a spline, bodies cut through sheet metal at a
//! slant, a sweep twisted along a line and a spline, an offset curve on a sphere, a draft both sides of a curved
//! parting sheet not matching, a variable conic fillet) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// None left.
const KNOWN_GAPS: &[&str] = &[];
/// The slanted cylinder's outline (c004): the same shape, but Onshape splits it at other points (its hole's faces
/// 2.31, 1.97, 26.99, 27.57 mm², the engine's 2.31, 2.31, 27.11, 27.11; 58.842 in all), its volume 0.0025 mm³ off.
/// The variable conic fillet (c009): Onshape's sections are its cubic fits of the conic, the end caps 0.003 mm²
/// and the volume 0.034 mm³ off (2e-6).
/// The tube along a spline (c002): the spline is the same (batch 073 c001), but Onshape's sweep along it is its own
/// fit, its faces up to 0.17 mm² and its volume 0.63 mm³ (3e-4) off the exact sweep's.
const LOOSE: &[(&str, f64)] = &[("c002", 0.7), ("c004", 0.003), ("c009", 0.04)];
/// The arc tube's cut list (c001): Onshape draws its iso curves round the tube, so finds no length and reports
/// angles of 7e-9 and 6e-8 rad; the engine's run along the tube (length 72.26 mm, square ends).
const BODIES_ONLY: &[&str] = &["c001", "c004"];
/// Faces come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c002", "c003", "c005", "c008", "c009"];

#[test]
fn batch_072_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-072");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, BODIES_ONLY, UNORDERED).unwrap();
        let problems: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|g| p.starts_with(g))).collect();
        assert!(problems.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (10, 10), "{report}");
    });
}
