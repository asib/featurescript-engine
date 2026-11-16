//! Builds fixture batch 183 (sweeps locked to faces with profiles of several loops) on the engine and compares it
//! with Onshape's.

use std::path::PathBuf;

/// Along the L's arc and along a spline Onshape's swept faces are its own fits (B-splines where the engine's are a
/// torus and cylinders, the spline's surfaces 0.5 mm² over the engine's exact sweep): only the bodies are compared,
/// loosely.
const BODIES_ONLY: &[&str] = &["c001", "c002", "c005"];
///
/// Up a half cylinder's edge (c004) the hole's side is Onshape's B-spline, but its edge on the caps comes out fitted
/// to the plane, the caps 0.0006 mm² under their exact areas: 0.006 mm³ of the volume.
const LOOSE: &[(&str, f64)] = &[("c001", 0.05), ("c002", 0.05), ("c004", 0.01), ("c005", 1.0)];

/// Onshape lists the faces in its own order.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004", "c005"];

#[test]
fn batch_183_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-183");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, BODIES_ONLY, UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (6, 6), "{report}");
    });
}
