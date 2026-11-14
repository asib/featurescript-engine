//! Builds fixture batch 173 (probes: the poles of B-spline face blends between a floor and a cylinder, and of swept
//! profile blends) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The faces' poles are Onshape's own representation (round the cylinder a cubic through the circle every 22.5°,
/// from 45°; the engine's blend is the exact revolved section): only the bodies are compared.
const BODIES_ONLY: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006", "c007"];

#[test]
fn batch_173_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-173");
        let report = fs_fixtures::check_batch_bodies(&batch, None, &[], BODIES_ONLY).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (8, 8), "{report}");
    });
}
