//! Builds fixture batch 154 (probes: dimensioned circles and tangent lines settling) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/FINDINGS.md): two circles without radius dimensions and two lines held tangent to
/// both (c005, batch 147's cam): Onshape keeps the first circle's radius and grows the second's (8 to 8.08), sliding
/// the other line straight up; the engine keeps both radii and turns the other line.
const KNOWN_GAPS: &[&str] = &["c005"];

#[test]
fn batch_154_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-154");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (7, 8), "{report}");
    });
}
