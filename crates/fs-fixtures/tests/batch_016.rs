//! Builds fixture batch 016 (a study of sheet metal bend and corner reliefs) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/std-coverage.md): corner reliefs, which the engine doesn't cut yet (c007-c013).
const KNOWN_GAPS: &[&str] = &["c007", "c008", "c009", "c010", "c011", "c012", "c013"];

#[test]
fn batch_016_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-016");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (14 - KNOWN_GAPS.len(), 14), "{report}");
    });
}
