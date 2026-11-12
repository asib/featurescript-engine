//! Builds fixture batch 142 (everyday parts: a pulley, a grooved shaft, a sheet metal bracket, a slotted plate, a
//! mirrored part, a snap box, a filleted pocket, a three-section loft) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The loft through a square, a circle and a square (c008): Onshape's sides are its own fits, its area 0.09 mm² and
/// its volume 0.26 mm³ (4e-5) over the engine's.
const LOOSE: &[(&str, f64)] = &[("c008", 0.3)];

#[test]
fn batch_142_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-142");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 9), "{report}");
    });
}
