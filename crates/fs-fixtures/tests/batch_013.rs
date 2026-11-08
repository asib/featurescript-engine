//! Builds fixture batch 013 (sheet metal bends, jogs, joints, corners, end, tabs, flange alignments) on the engine
//! and compares it with Onshape's.

use std::path::PathBuf;

/// The round corner relief folds into B-spline faces, which each kernel approximates its own way: its area and
/// volume are compared to 0.05 (mm² and mm³, about 1e-5 of them) (c006).
const LOOSE: &[(&str, f64)] = &[("c006", 0.05)];

#[test]
fn batch_013_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-013");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (12, 12), "{report}");
    });
}
