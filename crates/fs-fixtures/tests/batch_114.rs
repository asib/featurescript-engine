//! Builds fixture batch 114 (curvature fillets past 0.975 at 60°, and at 45°, 75° and 135° corners) on the engine
//! and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): at 0.99875, where the section dips
/// past the faces by nanometres, the engine's cut leaves two edges more than Onshape's 15 (c019); at 45°, 75° and
/// 135° corners past 0.975, where Onshape's section goes with the angle in a way not found and isn't sampled
/// (c020–c022).
const KNOWN_GAPS: &[&str] = &["c019", "c020", "c021", "c022"];

/// Where OpenCascade can't compare the parts: their fillet's section turns through the corner within micrometres,
/// and OCCT's distances and booleans fail on such a surface, so the difference check reports nonsense. Onshape's own
/// volume, area and box of them match (to 2e-5).
const UNCOMPARABLE: &[&str] = &["c018.1: the shapes differ"];

/// Curvature fillets at a 60° corner from 0.975 to 0.9975, their sections Onshape's own, sampled: its areas and
/// volumes of its quintics stray, as in batch 053 (within 0.06 mm³, 1.1e-5 of the volume).
const LOOSE: &[(&str, f64)] = &[
    ("c001", 0.1),
    ("c002", 0.1),
    ("c003", 0.1),
    ("c004", 0.1),
    ("c005", 0.1),
    ("c006", 0.1),
    ("c007", 0.1),
    ("c008", 0.1),
    ("c009", 0.1),
    ("c010", 0.1),
    ("c011", 0.1),
    ("c012", 0.1),
    ("c013", 0.1),
    ("c014", 0.1),
    ("c015", 0.1),
    ("c016", 0.1),
    ("c017", 0.1),
    ("c018", 0.1),
];

/// The probes return Onshape's poles: only the bodies are compared.
const BODIES_ONLY: &[&str] = &[
    "c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008", "c009", "c010", "c011", "c012", "c013", "c014",
    "c015", "c016", "c017", "c018", "c019", "c020", "c021", "c022",
];

#[test]
fn batch_114_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-114");
        let report = fs_fixtures::check_batch_bodies(&batch, None, LOOSE, BODIES_ONLY).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().chain(UNCOMPARABLE).any(|case| p.starts_with(case)) && !p.starts_with("failed probe"))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (23 - KNOWN_GAPS.len() - 1, 23), "{report}");
    });
}
