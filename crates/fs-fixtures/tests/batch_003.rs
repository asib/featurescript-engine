//! Builds fixture batch 003 (fillet, chamfer, shell, draft and face edits) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// The conic (c013) and asymmetric (c014) fillets' surfaces are the cubic B-splines Onshape exports, to the area
/// (70.451358 and 57.528781 mm² as OCCT measures both), but Onshape's own volumes and areas of them stray from any
/// exact value by up to 6e-6 (c014's volume 0.038 mm³ above the B-spline's, 0.022 above the true quarter
/// ellipse's; c013's 0.016 and 0.006 below), so their probes are compared to 0.05.
const LOOSE: &[(&str, f64)] = &[("c013", 0.05), ("c014", 0.05)];

#[test]
fn batch_003_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-003");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (56, 56), "{report}");
    });
}
