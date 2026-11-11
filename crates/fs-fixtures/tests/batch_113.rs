//! Builds fixture batch 113 (curvature fillets between the sampled magnitudes past 0.979, and at 60° and 120°
//! corners) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): a curvature fillet of magnitude
/// 0.99875, its section within micrometres of the corner, too thin a tool for the engine's booleans (c009); at 60°
/// and 120° corners, where past 0.975 Onshape's section differs from a right angle's and isn't sampled (c010,
/// c011).
const KNOWN_GAPS: &[&str] = &["c009", "c010", "c011"];

/// Where OpenCascade can't compare the parts: their fillet's section turns through the corner within micrometres,
/// and OCCT's distances and booleans fail on such a surface (a point on the face measures 4 mm from it), so the
/// difference check reports nonsense. Onshape's own volume, area and box of them match (to 3e-6).
const UNCOMPARABLE: &[&str] = &["c003.1: the shapes differ"];

/// Curvature fillets of magnitude 0.97875 to 0.99625, their sections Onshape's own, sampled: its areas and volumes
/// of its quintics stray, as in batch 053 (within 0.02).
const LOOSE: &[(&str, f64)] = &[
    ("c001", 0.05),
    ("c002", 0.05),
    ("c003", 0.05),
    ("c004", 0.05),
    ("c005", 0.05),
    ("c006", 0.05),
    ("c007", 0.05),
    ("c008", 0.05),
];

/// The probes return Onshape's poles: only the bodies are compared.
const BODIES_ONLY: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008", "c009", "c010", "c011"];

#[test]
fn batch_113_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-113");
        let report = fs_fixtures::check_batch_bodies(&batch, None, LOOSE, BODIES_ONLY).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().chain(UNCOMPARABLE).any(|case| p.starts_with(case)) && !p.starts_with("failed probe"))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (12 - KNOWN_GAPS.len() - 1, 12), "{report}");
    });
}
