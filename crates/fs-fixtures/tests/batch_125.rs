//! Builds fixture batch 125 (helix sweeps square to the path, sphere rims, a cone edge, a hemisphere shelled, a
//! cross hole filleted) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): a cross hole's mouth through a
/// lying cylinder filleted, OCCT's fillet two lopsided B-spline faces where Onshape's is one (c008: areas 5.3 mm²
/// apart).
const KNOWN_GAPS: &[&str] = &["c008"];

/// Sweeps along helices (c001–c003): Onshape's helix is its own fit, micrometres off the engine's, so areas differ
/// by up to 0.004 mm².
const LOOSE: &[(&str, f64)] = &[("c001", 0.01), ("c002", 0.01), ("c003", 0.01)];

#[test]
fn batch_125_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        // The helix sweeps lie on each other in places and apart in others: OCCT's boolean between them runs for
        // many minutes, so they're compared by their measures alone.
        fs_fixtures::skip_shape_difference(&["c001.1", "c002.1", "c003.1"]);
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-125");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| {
                !KNOWN_GAPS.iter().any(|case| p.starts_with(case))
                    && !p.starts_with("engine STEP: c008.1")
            })
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        // Every probe line but the gaps'.
        assert_eq!(report.probes_matched, (9 - KNOWN_GAPS.len(), 9), "{report}");
    });
}
