//! Builds fixture batch 084 (hold lines and a cliff edge inside a side, fillets of circular edges on a cone and round
//! a boss, a conic blend with a cylinder) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The curvature fillet round the boss and the conic blend round the cylinder are Onshape's fits: areas to 1.7e-5,
/// and the blend's edge on the floor encloses 0.1 mm² more than the exact circle.
const LOOSE: &[(&str, f64)] = &[("c006", 0.15), ("c007", 0.15)];

/// Faces come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004", "c006", "c007"];

/// c005's revolve about the profile's own sketch edge, found by a point on it (the sketch's line and its region's
/// edge both), fails in both (`REVOLVE_SELECT_AXIS`, batch 089).
const KNOWN_GAPS: &[&str] = &[];

#[test]
fn batch_084_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-084");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|gap| p.starts_with(gap))).collect();
        assert!(unexpected.is_empty(), "{report}");
    });
}
