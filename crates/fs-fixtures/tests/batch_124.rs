//! Builds fixture batch 124 (everyday modelling: springs, a boss up to a curved face, a pocket's rim, drafts round
//! fillets, a tee) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): the tee's joint chamfered,
/// Onshape's 1 mm measured otherwise along the curve between two cylinders (c008: 5.6 mm³ apart).
const KNOWN_GAPS: &[&str] = &["c008"];

/// The bowl's two spheres from two kernels, a hair apart, defeat OCCT's boolean between them (it reports the
/// whole volume); Onshape's volume, area and box of it match.
const UNCOMPARABLE: &[&str] = &["c010.1: the shapes differ"];

/// Sweeps along a helix (c001, c002): Onshape's helix is its own fit, micrometres off the engine's (7 µm between
/// the springs' surfaces), so areas differ by up to 0.004 mm². (The profiles lie 18° off square to the path, the
/// generator having taken the helix's hand the wrong way round; both sweep them alike.)
const LOOSE: &[(&str, f64)] = &[("c001", 0.01), ("c002", 0.01)];

#[test]
fn batch_124_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        // The helix sweeps lie on each other in places and apart in others: OCCT's boolean between them runs for
        // many minutes, so they're compared by their measures alone.
        fs_fixtures::skip_shape_difference(&["c001.1", "c002.1"]);
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-124");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| {
                !KNOWN_GAPS.iter().chain(UNCOMPARABLE).any(|case| p.starts_with(case))
                    && !p.starts_with("engine STEP: c008.1")
            })
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        // Every probe line but the gaps'.
        assert_eq!(report.probes_matched, (11 - KNOWN_GAPS.len(), 11), "{report}");
    });
}
