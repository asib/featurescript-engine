//! Builds fixture batch 024 (tapped, clearance and V3 holes, revolve start bounds, an aligned gusset, ruled surface
//! vertex overrides, a planar fill and a wrap onto a cone) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_024_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-024");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (12, 12), "{report}");
    });
}
