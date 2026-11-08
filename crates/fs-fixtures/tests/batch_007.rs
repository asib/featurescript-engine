//! Builds fixture batch 007 (surface coordinate systems, selection queries, and more options of hole, rib, body
//! draft, revolve, split face, transform, isocline, name entity and query variable) on the engine and compares it
//! with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/std-coverage.md): the coordinate systems of a fillet's cylinder and of a hole cut
/// through all, whose origin sits at Onshape's own through-all depth (c007).
const KNOWN_GAPS: &[&str] = &["c007"];

#[test]
fn batch_007_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-007");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (28 - KNOWN_GAPS.len(), 28), "{report}");
    });
}
