//! Builds fixture batch 112 (Onshape's guided and pathed lofts, sphere extensions and a rolled wall's hole, probed
//! for their poles) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine and Onshape don't agree on (see docs/std-coverage.md): circles lofted along a straight path
/// (c004–c006: Onshape's side is the cone along its height, but its sections between the ends pass through the
/// circle only every 45°, dipping up to 0.006 mm between); a sphere's band extended round it (c007, c008: Onshape's
/// new edge depends on the whole band); a hole cut across a rolled wall (c009: Onshape's wall is its own adaptive
/// fit, with two planar slivers).
const KNOWN_GAPS: &[&str] = &["c004", "c005", "c006", "c007", "c008", "c009"];

/// Lofts with a guide crossing a side: Onshape's surface is the engine's to within its own fit of the guide's arc
/// (1e-4 to 5e-4 mm), the volumes within 1.8e-6, 7.8e-6 (c002, its fit the coarsest) and 3.2e-6 (c001–c003).
const LOOSE: &[(&str, f64)] = &[("c001", 0.02), ("c002", 0.05), ("c003", 0.02)];

/// The probes return Onshape's poles, its own fits: only the bodies are compared.
const BODIES_ONLY: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008", "c009"];

#[test]
fn batch_112_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-112");
        let report = fs_fixtures::check_batch_bodies(&batch, None, LOOSE, BODIES_ONLY).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (10 - KNOWN_GAPS.len(), 10), "{report}");
    });
}
