//! Builds fixture batch 013 (sheet metal bends, jogs, joints, corners, end, tabs, flange alignments) on the engine
//! and compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/std-coverage.md): the jog, whose second split std finds badly decomposed (c003),
/// and the round corner relief, which Onshape cuts in the flat pattern and folds back (c006).
const KNOWN_GAPS: &[&str] = &["c003", "c006"];

/// Cases whose parts match but whose body counts differ, because the cases above leave no bodies behind: their
/// probe lines aren't compared (their parts are).
const COUNTS_ONLY: &[&str] = &["c004", "c005", "c007", "c008", "c009", "c010", "c011"];

#[test]
fn batch_013_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-013");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case)))
            .filter(|p| {
                !COUNTS_ONLY.iter().any(|case| {
                    p.starts_with(&format!("{case} probe: ")) && (p.contains(" active: ") || p.contains(" sheets: "))
                })
            })
            .filter(|p| !p.starts_with("failed probe"))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched.1, 12, "{report}");
    });
}
