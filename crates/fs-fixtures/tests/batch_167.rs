//! Builds fixture batch 167 (face patterns of bosses and pockets together; surfaces extruded up to a curved face in
//! their second direction) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_167_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-167");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (5, 5), "{report}");
    });
}
