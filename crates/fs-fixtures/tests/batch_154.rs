//! Builds fixture batch 154 (probes: dimensioned circles and tangent lines settling) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/FINDINGS.md): two lines held tangent to circles drawn a little off (c003–c007).
/// Onshape keeps the lower line where it was, whichever was drawn or constrained first, moves the circles the
/// shortest way to it, and moves the other line onto them (along its normal for one circle, turned about its middle
/// for two); the engine keeps both lines and moves the circles to where both touch them.
const KNOWN_GAPS: &[&str] = &["c003", "c004", "c005", "c006", "c007"];

#[test]
fn batch_154_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-154");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (3, 8), "{report}");
    });
}
