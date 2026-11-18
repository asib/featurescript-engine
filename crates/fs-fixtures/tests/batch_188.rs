//! Builds fixture batch 188 (lofts of other profiles) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Onshape's ends measured on other approximations than its sides' edges: a connected loft's (c002, c008: 0.009 and
/// 0.006 mm²), the ellipse's (c005: 0.0016 mm²).
const LOOSE: &[(&str, f64)] = &[("c002", 0.05), ("c005", 0.05), ("c008", 0.05)];

/// Onshape lists the faces in its own order.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008", "c009"];

#[test]
fn batch_188_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-188");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (10, 10), "{report}");
    });
}
