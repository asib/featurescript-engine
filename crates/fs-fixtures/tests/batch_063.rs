//! Builds fixture batch 063 (curvature fillets from magnitude 0.98 to 1, polygon lofts normal to a profile, a fill
//! of arcs, boundary surfaces of arcs) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): a fill of four arcs round a
/// non-planar loop, Onshape's own fit (c008). A boundary surface tangent to a profile fails in both
/// (`CANNOT_MAKE_BSPLINESURFACE`, c010).
const KNOWN_GAPS: &[&str] = &["c008"];

/// Where OpenCascade can't compare the parts: their fillet's section turns through the corner within micrometres,
/// and OCCT's distances and booleans fail on such a surface (a point on the face measures 4 mm from it), so the
/// difference check reports nonsense. Onshape's own volume, area and box of them match (to 3e-6).
const UNCOMPARABLE: &[&str] = &["c002.1: the shapes differ", "c003.1: the shapes differ"];

/// Polygon lofts normal to a profile: the derivative at the corners is the magnitude times 0.9 of the chord along
/// the normal plus 0.1 of its length, as the engine has it, but along the edges Onshape blends it by a rule not
/// found (its sides are fits of eight spans): the volumes are within 2e-5 (c005–c007).
/// Boundary surfaces of arcs, a Coons patch (c009) and normal to a profile (c011): Onshape's are built on its own
/// eight-span fits of the arcs, 0.004 and 0.001 mm² off the engine's exact ones.
/// Curvature fillets of magnitude 0.98 to 0.995, their sections Onshape's own, sampled (c001–c003): its areas and
/// volumes of its quintics stray, as in batch 053 (within 0.02).
const LOOSE: &[(&str, f64)] = &[
    ("c001", 0.05),
    ("c002", 0.05),
    ("c003", 0.05),
    ("c005", 0.1),
    ("c006", 0.1),
    ("c007", 0.1),
    ("c009", 0.01),
    ("c011", 0.005),
];

/// The fillets', lofts' sides and the boundary surfaces are Onshape's fits: only their bodies are compared.
const BODIES_ONLY: &[&str] = &["c001", "c002", "c003", "c005", "c006", "c007", "c009", "c011"];

#[test]
fn batch_063_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-063");
        let report = fs_fixtures::check_batch_bodies(&batch, None, LOOSE, BODIES_ONLY).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().chain(UNCOMPARABLE).any(|case| p.starts_with(case)) && !p.starts_with("failed probe"))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        // Every probe line but the gaps', the failed-cases line too (c004 and c010 fail in both).
        assert_eq!(report.probes_matched, (12 - KNOWN_GAPS.len(), 12), "{report}");
    });
}
