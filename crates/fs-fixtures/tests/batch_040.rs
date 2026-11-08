//! Builds fixture batch 040 (ruled surfaces normal to a curved face along its edge) on the engine and compares it
//! with Onshape's.

use std::path::PathBuf;

/// Onshape's far edges are approximations: its sheets' areas are within 0.005 mm² of the exact regions, which the
/// engine's match to 3e-6 (an annular sector 98.174770 mm²: Onshape 98.169686, the engine 98.174768).
const LOOSE: &[(&str, f64)] = &[("c001", 0.01), ("c002", 0.01), ("c003", 0.01)];

#[test]
fn batch_040_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-040");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (4, 4), "{report}");
    });
}
