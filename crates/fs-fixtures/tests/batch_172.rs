//! Builds fixture batch 172 (face blend options between a floor and an upright cylinder, chamfers, swept profiles)
//! on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Onshape's B-spline blends round the cylinder (two radii, curvature) leave the floor a hole whose edge is a curve
/// 1e-4 longer than its circle, the floor 0.11–0.14 mm² smaller than the engine's exactly round one (as batch 084
/// c007's conic); its blend's own surface is a cubic round the axis, 0.03 mm² under the engine's exact one.
const LOOSE: &[(&str, f64)] = &[("c002", 0.15), ("c006", 0.15), ("c007", 0.15)];

/// Onshape lists the sewn sheet's faces in its own order.
const UNORDERED: &[&str] = &["c002", "c003", "c004", "c006", "c007", "c008", "c009", "c010", "c011"];

#[test]
fn batch_172_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-172");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (12, 12), "{report}");
    });
}
