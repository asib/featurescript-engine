//! Builds fixture batch 054 (face blends: sections, uneven radii, width, trims, sheets apart and at angles) on the
//! engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_054_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-054");
        let report = fs_fixtures::check_batch_with(&batch, None, &[]).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (13, 13), "{report}");
    });
}
