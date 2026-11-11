//! Builds fixture batch 107 (a sheet metal plate drawn in Onshape's UI: built, mirrored with derivedMirror, its zlib-compressed query decoded) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_107_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-107");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], &[]).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (4, 4), "{report}");
    });
}
