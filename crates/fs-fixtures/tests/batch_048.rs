//! Builds fixture batch 048 (sweeps twisted and scaled along curved paths) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Onshape's twisted and scaled sweeps along an arc lie up to about a micrometre inside the profile carried along the
/// path, as along a straight one (batch 019), and the engine's surfaces pass through it: their volumes differ by
/// up to 3.2e-4 (0.36 mm³ of the twisted rectangle's 1131), so the probes are compared to 0.5 and the surfaces,
/// where a boolean can't tell them apart, loosely.
const LOOSE: &[(&str, f64)] = &[("c001", 0.5), ("c002", 0.5), ("c003", 0.5)];

#[test]
fn batch_048_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-048");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (4, 4), "{report}");
    });
}
