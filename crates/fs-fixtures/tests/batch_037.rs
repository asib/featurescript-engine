//! Builds fixture batch 037 (section parts for crop views, offset sections and aligned sections) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

/// Where Onshape puts the origin of a planar sheet that isn't symmetric (docs/std-coverage.md, batch 024): c004's
/// aligned section leaves a piece of a default plane's rotated copy, whose origin is 0.3% of its box off the middle
/// of its box in the copy's own axes. The sheet itself matches.
const KNOWN_GAPS: &[&str] = &["c004 probe: c004.3 plane"];

#[test]
fn batch_037_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-037");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|gap| p.starts_with(gap))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (4, 5), "{report}");
    });
}
