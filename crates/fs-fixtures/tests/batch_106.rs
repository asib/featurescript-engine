//! Builds fixture batch 106 (std's query functions no batch had evaluated: unique vertices, a tolerance filter, touching a line, within a radius, a coedge, a mesh source, an axis) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_106_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-106");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], &[]).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (8, 8), "{report}");
    });
}
