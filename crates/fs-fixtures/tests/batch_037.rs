//! Builds fixture batch 037 (section parts for crop views, offset sections and aligned sections) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine and Onshape don't agree on (see docs/std-coverage.md): none. c004's aligned section leaves a
/// piece of a default plane's rotated copy bounded by a curve, whose plane's origin takes the curve's control points
/// into its box, as Onshape's does (batch 116).
const KNOWN_GAPS: &[&str] = &[];

#[test]
fn batch_037_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-037");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|gap| p.starts_with(gap))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (5, 5), "{report}");
    });
}
