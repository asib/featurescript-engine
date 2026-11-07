//! Builds fixture batch 003 (fillet, chamfer, shell, draft and face edits) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build yet (see docs/std-coverage.md): a conic fillet, an asymmetric fillet, and a fillet
/// too big for the faces it rounds, which Onshape lets eat a whole face.
const KNOWN_GAPS: &[&str] = &["c013", "c014", "c015"];

#[test]
fn batch_003_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-003");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case)) && !p.starts_with("failed probe"))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (56 - KNOWN_GAPS.len() - 1, 56), "{report}");
    });
}
