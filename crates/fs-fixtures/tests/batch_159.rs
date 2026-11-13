//! Builds fixture batch 159 (boundary surfaces matching the tangency of flat faces next to their profiles; which way
//! flat fills face; which way an extruded sheet's edges run) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// From a level sheet's edge up to an arc (c005, and as v profiles c014): Onshape's arc is its own fit, the surface's
/// area 0.0015 mm² and its heights 2.2e-4 mm off the engine's on the exact arc.
const LOOSE: &[(&str, f64)] = &[("c005", 0.005), ("c014", 0.005)];

#[test]
fn batch_159_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-159");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (15, 15), "{report}");
    });
}
