//! Builds fixture batch 187 (more lofts with connections; a slot and an ellipse lofted to a circle) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

/// Onshape's ends of connected lofts measured on other approximations of the curves than its sides' edges (as in
/// batch 180): c005–c009's circles; the slot's and the ellipse's too (c010: 0.002 mm² and 0.03 mm³; c011: the ellipse's
/// 0.03 mm², the circle's 0.025 as batch 180's, 0.18 mm³).
const LOOSE: &[(&str, f64)] = &[
    ("c005", 0.002),
    ("c006", 0.002),
    ("c007", 0.002),
    ("c008", 0.05),
    ("c009", 0.002),
    ("c010", 0.05),
    ("c011", 0.2),
];

/// Onshape lists the faces in its own order.
const UNORDERED: &[&str] =
    &["c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008", "c009", "c010", "c011", "c012", "c013"];

#[test]
fn batch_187_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-187");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (14, 14), "{report}");
    });
}
