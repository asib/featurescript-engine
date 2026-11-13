//! Builds fixture batch 147 (everyday parts, an eighth round: a cam, a boss up to a tilted plane, a 270° ring, a
//! drafted tray, a shaft collar, a swept handle, holes in a slanted face) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/FINDINGS.md). The cam (c001): its lines drawn a little off and held tangent to two
/// circles with nothing dimensioned, Onshape's solver keeps the lines' directions and the first circle's radius,
/// lengthens the lines and grows the second circle (8 to 8.08), all shifted 0.05 up; the engine's keeps both radii
/// and the lines' lengths and turns the second line.
const KNOWN_GAPS: &[&str] = &["c001"];

#[test]
fn batch_147_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-147");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (7, 8), "{report}");
    });
}
