//! Builds fixture batch 190 (rounded rectangles lofted to circles) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Onshape's ends bounded by other approximations than its side's edges where it splits its circle (c005: 0.005 mm³).
const LOOSE: &[(&str, f64)] = &[("c005", 0.01)];

/// Onshape lists the faces in its own order.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006"];

#[test]
fn batch_190_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-190");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (7, 7), "{report}");
    });
}
