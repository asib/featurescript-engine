//! Builds fixture batch 016 (a study of sheet metal bend and corner reliefs) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Corner reliefs and the closed corner fold B-spline faces, which each kernel approximates its own way: areas and
/// volumes are compared to 0.01 (mm² and mm³, about 2e-6 of them).
const LOOSE: &[(&str, f64)] = &[("c007", 0.01), ("c008", 0.01), ("c009", 0.01), ("c011", 0.01), ("c013", 0.01)];

#[test]
fn batch_016_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-016");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (14, 14), "{report}");
    });
}
