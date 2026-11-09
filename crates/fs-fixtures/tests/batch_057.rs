//! Builds fixture batch 057 (lofts to points and matching faces, sweeps twisted along a line and an arc, a torus
//! sheet extended, tilted torus isoclines) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build yet (see docs/std-coverage.md): lofts matching the tangency or curvature of the
/// faces next to the start profile (c005–c008).
const KNOWN_GAPS: &[&str] = &["c005", "c006", "c007", "c008"];

/// A loft from a square to a point, normal to the square: Onshape's sides are fits that bulge between the corners
/// and the middles of the edges by up to 0.003 mm (0.28 mm³ of volume). Sweeps twisted or scaled along a line and
/// an arc: as along an arc alone (batch 048), Onshape's surfaces are approximations, within 7e-5 of the volume.
const LOOSE: &[(&str, f64)] = &[("c004", 0.3), ("c009", 0.5), ("c010", 0.5)];

/// Every case returns its faces' surfaces or points along its edges, to study. Only the bodies are compared.
const CASES: &[&str] =
    &["c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008", "c009", "c010", "c011", "c012", "c013"];

#[test]
fn batch_057_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-057");
        let report = fs_fixtures::check_batch_bodies(&batch, None, LOOSE, CASES).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case)) && !p.starts_with("failed probe"))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (14 - KNOWN_GAPS.len() - 1, 14), "{report}");
    });
}
