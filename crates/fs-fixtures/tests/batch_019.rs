//! Builds fixture batch 019 (sweeps twisted and scaled, ribs, end caps, ruled surfaces, curve patterns, projection,
//! sheet metal reliefs and unfolds) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Onshape's twisted sweep is itself up to 0.8 µm off the profile turned along the path (the engine's surfaces pass
/// through it), so where a boolean can't tell the two apart their surfaces are compared to 5 µm (c001).
const LOOSE: &[(&str, f64)] = &[("c001", 1e-3)];

#[test]
fn batch_019_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-019");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (14, 14), "{report}");
    });
}
