//! Builds fixture batch 141 (everyday parts: a hole grid, a mirrored gusset, a lofted transition, a sheet metal
//! channel, a split block, an intersection, a thickened loft, countersinks) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// The transition's shell (c003): the engine's cavity is the sides' exact offsets (its volume 3278.5712 mm³, the
/// exact 3278.5696 found apart, its end rings to 1e-7 of theirs), Onshape's its own fits of them, its volume 0.52 mm³
/// over (and its STEP's 1.2 under, its bottom ring 0.27 mm² short); so its measures are compared loosely.
const LOOSE: &[(&str, f64)] = &[("c003", 0.6)];

#[test]
fn batch_141_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-141");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], &[]).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 9), "{report}");
    });
}
