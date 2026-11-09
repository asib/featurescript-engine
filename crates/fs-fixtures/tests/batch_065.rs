//! Builds fixture batch 065 (sweeps twisted and scaled along a spline, an isocline on lofted B-spline faces,
//! extending a lofted sheet, an offset curve on a sphere, an arc trimmed by a circle) on the engine and compares
//! it with Onshape's.

use std::path::PathBuf;

/// Cases the engine and Onshape don't agree on (see docs/std-coverage.md): an offset curve up a sphere from its
/// equator, which both fail, with different errors (Onshape's "Execution error", c005).
const KNOWN_GAPS: &[&str] = &["c005"];

/// Sweeps along a spline: the engine lofts through copies carried by the rotation-minimising frame, within 1.0e-4
/// (twisted) and 2.5e-4 (scaled) of Onshape's volumes (c001, c002). The extended sheet: Onshape's surface and new
/// edge are its fits of the arcs and of the rulings carried on, within 0.0014 mm of the engine's (c004).
const LOOSE: &[(&str, f64)] = &[("c001", 0.12), ("c002", 0.15), ("c004", 0.03)];

/// The sweeps' and the extended sheet's faces are Onshape's fits: only their bodies are compared.
const BODIES_ONLY: &[&str] = &["c001", "c002", "c004"];

/// The isocline's edges come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c003"];

#[test]
fn batch_065_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-065");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, BODIES_ONLY, UNORDERED).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case)) && !p.starts_with("failed probe"))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (7 - KNOWN_GAPS.len(), 7), "{report}");
    });
}
