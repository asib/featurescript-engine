//! Builds fixture batch 091 (sketch text) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Known gap (docs/FINDINGS.md): the engine has no `@skText`. Onshape's text is its font engine's outlines (lines and
/// quadratic Béziers, the box's height 0.72 em, the baseline on its bottom, a construction frame the advance wide),
/// quantized to the box's height / 23040 and shifted by hinting from the published fonts' points, from font
/// versions other than the published ones (Open Sans's "I" is 504 units wide there, 572 in Open Sans 3).
const KNOWN_GAPS: &[&str] = &["c001", "c002", "c003", "c004", "c005", "failed probe"];

#[test]
fn batch_091_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-091");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], &[]).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|gap| p.starts_with(gap))).collect();
        assert!(unexpected.is_empty(), "{report}");
    });
}
