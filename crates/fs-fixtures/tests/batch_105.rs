//! Builds fixture batch 105 (std's last builtins through the std functions that call them: a tolerance schema's validation, mesh points, a sheet metal derip) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_105_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-105");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], &[]).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (10, 10), "{report}");
    });
}
