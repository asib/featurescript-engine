//! Builds fixture batch 120 (geodesic offset curves across a sphere from a meridian and across a cone from a ruling)
//! on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The cone's offset (c002): the engine's is the exact geodesic offset (the cone developed into the plane, a line 3
/// mm from the ruling, 20.5063 mm long down to the bottom edge); Onshape's is its own fit, 0.006 mm off it.
const LOOSE: &[(&str, f64)] = &[("c002", 0.01)];

/// The probes return the curves' points and poles: only the bodies are compared.
const BODIES_ONLY: &[&str] = &["c001", "c002"];

#[test]
fn batch_120_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-120");
        let report = fs_fixtures::check_batch_bodies(&batch, None, LOOSE, BODIES_ONLY).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (3, 3), "{report}");
    });
}
