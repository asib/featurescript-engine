//! Builds fixture batch 039 (curved sheets other than cylinders extended) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// A sphere extended round (docs/std-coverage.md): Onshape's new edge turns further than the distance round each
/// circle, by a rule not known yet (c003).
const KNOWN_GAPS: &[&str] = &["c003 probe"];

/// Onshape's new edges on a cone are its own approximations of the edge carried the distance along the surface:
/// round it, the area 0.005 mm² short of that (c001); along it, the edge up to 0.013 mm further out and the area
/// 0.027 mm² over (c002). Compared to 0.05.
const LOOSE: &[(&str, f64)] = &[("c001", 0.05), ("c002", 0.05), ("c004", 0.05)];

#[test]
fn batch_039_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-039");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|gap| p.starts_with(gap))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 10), "{report}");
    });
}
