//! Builds fixture batch 013 (sheet metal bends, jogs, joints, corners, end, tabs, flange alignments) on the engine
//! and compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/std-coverage.md): the jog, whose second split std finds badly decomposed (c003).
const KNOWN_GAPS: &[&str] = &["c003", "failed probe"];

/// The round corner relief folds into B-spline faces, which each kernel approximates its own way: its area is
/// compared to 0.05 (mm² and mm³, about 1e-5 of them) (c006).
const LOOSE: &[(&str, f64)] = &[("c006", 0.05)];

#[test]
fn batch_013_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-013");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (10, 12), "{report}");
    });
}
