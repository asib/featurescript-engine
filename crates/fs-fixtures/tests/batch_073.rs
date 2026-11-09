//! Builds fixture batch 073 (sketch fit splines, isoparametric curves on a torus, variable curvature, asymmetric and
//! conic fillets) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The variable fillets: Onshape's sections are its cubic fits of conics and curvature sections, its end faces up to
/// 0.013 mm² and volumes up to 0.094 mm³ (7e-6) off the exact surfaces'; the fillet faces' areas match to 1e-7.
const LOOSE: &[(&str, f64)] = &[("c006", 0.1), ("c007", 0.07), ("c008", 0.04), ("c009", 0.04)];
/// The torus's isoparametric curves: the same curves, but Onshape's run the other way round its axis and its tube
/// circle starts on the inside (its torus's parameters run otherwise than OpenCascade's); their periodicity matches.
const BODIES_ONLY: &[&str] = &["c004", "c005"];
/// Faces come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c006", "c007", "c008", "c009"];

#[test]
fn batch_073_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-073");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, BODIES_ONLY, UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (10, 10), "{report}");
    });
}
