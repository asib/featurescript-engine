//! Builds fixture batch 145 (everyday parts, a sixth round: a nut, a bolt, a hinge leaf, a spoked wheel, a bottle, a
//! ribbed bracket, a box lip, a notched knob) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The bottle (c005), a loft through three circles shelled: Onshape's sides are its own fits, its area 0.024 mm² and
/// its volume 0.031 mm³ (4e-6) under the engine's.
const LOOSE: &[(&str, f64)] = &[("c005", 0.04)];

#[test]
fn batch_145_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-145");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 9), "{report}");
    });
}
