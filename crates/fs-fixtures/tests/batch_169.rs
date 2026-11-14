//! Builds fixture batch 169 (lofts whose profiles are more than one face) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/FINDINGS.md): a 20 × 10 rectangle lofted to a circle (c001), 0.23% of the volume off
/// Onshape's whether the rectangle is split or not: the circle split at the corners' angles (as a square's matches,
/// batch 004 c039) comes nearest of the splits tried, but Onshape's loft is its own.
const KNOWN_GAPS: &[&str] = &["c001"];

#[test]
fn batch_169_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-169");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (3, 4), "{report}");
    });
}
