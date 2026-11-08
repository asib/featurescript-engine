//! Builds fixture batch 039 (curved sheets other than cylinders extended) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Cones and spheres extended (c001–c004; docs/std-coverage.md): Onshape carries the surface on, but how it measures
/// the distance round or along it isn't known yet, so the engine doesn't extend them.
const KNOWN_GAPS: &[&str] = &["c001 probe", "c002 probe", "c003 probe", "c004 probe", "failed probe"];

#[test]
fn batch_039_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-039");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|gap| p.starts_with(gap))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (5, 10), "{report}");
    });
}
