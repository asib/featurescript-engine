//! Builds fixture batch 164 (revolves bounded at both ends, extrudes up to curved faces in their second direction,
//! sheet metal thickened with tangent propagation) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// A circle extruded down to a lying cylinder (c004): Onshape's box of the solid stops 1.6e-3 mm short of its
/// lowest point (where the extrude meets the cylinder, z = -14.70850), presumably bounding its fit of that curve;
/// the engine's reaches it.
const LOOSE: &[(&str, f64)] = &[("c004", 0.005)];

#[test]
fn batch_164_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-164");
        let step = batch.join("onshape.step");
        let report = fs_fixtures::check_batch_with(&batch, Some(&step), LOOSE).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (8, 8), "{report}");
    });
}
