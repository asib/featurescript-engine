//! Builds fixture batch 151 (everyday parts, a tenth round: revolve up to a plane, face patterns and mirrors, up to a
//! part with an offset, an intersection, a thickened side, a split and delete, a propagated draft) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_151_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-151");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 9), "{report}");
    });
}
