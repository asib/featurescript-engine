//! Builds fixture batch 058 (curved draft parting, faces replaced by an off-axis cylinder and a slanted plane, a
//! sweep locked to a cylinder, a tilted torus split, lofts matching slanted faces) on the engine and compares it
//! with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): a tilted torus split along its
/// isoclines, which cross the torus's seams (c005).
const KNOWN_GAPS: &[&str] = &["c005"];

/// Lofts from a frustum's top matching its leaning sides: Onshape's sides, a cubic along each edge, bulge between
/// the corners (0.03 mm³ to the smaller square; 0.64, 1.4e-4 of the volume, to the square of the same size).
const LOOSE: &[(&str, f64)] = &[("c006", 0.05), ("c007", 0.7)];

/// Every case returns its faces' surfaces, to study. Only the bodies are compared.
const CASES: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008"];

#[test]
fn batch_058_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-058");
        let report = fs_fixtures::check_batch_bodies(&batch, None, LOOSE, CASES).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case)) && !p.starts_with("failed probe"))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9 - KNOWN_GAPS.len() - 1, 9), "{report}");
    });
}
