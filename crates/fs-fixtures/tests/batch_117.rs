//! Builds fixture batch 117 (boundary surfaces between arcs with end conditions at both profiles, the second, and
//! along directions) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine doesn't build as Onshape does yet (see docs/std-coverage.md): end conditions of magnitude 2
/// (c002) and 0.5 and 1.5 (c006), whose derivatives aren't the chord's parts scaled, and along a direction given
/// (c004), whose free end isn't the quadratic's; the engine refuses them, so the failed-cases line differs too.
const KNOWN_GAPS: &[&str] = &["c002", "c004", "c006", "failed probe"];

/// Normal to both profiles' planes (c001) and to the second's (c003): Onshape's surfaces are built on its own fits
/// of the arcs, 0.0002 and 0.0015 mm² off the engine's exact ones.
const LOOSE: &[(&str, f64)] = &[("c001", 0.005), ("c003", 0.005)];

/// The probes return Onshape's poles, its own fits: only the bodies are compared.
const BODIES_ONLY: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006"];

#[test]
fn batch_117_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-117");
        let report = fs_fixtures::check_batch_bodies(&batch, None, LOOSE, BODIES_ONLY).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        // c001, c003 and c005 (failing in both, BSURF_INCONSISTENT_CONDITIONS_ERROR).
        assert_eq!(report.probes_matched, (3, 7), "{report}");
    });
}
