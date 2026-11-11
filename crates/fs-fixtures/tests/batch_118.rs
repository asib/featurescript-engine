//! Builds fixture batch 118 (curved faces replaced: spheres by spheres, a cylinder by itself offset and by a sphere)
//! on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The barrel's faces (c004) come in an order of Onshape's own: its caps first.
const UNORDERED: &[&str] = &["c004"];

#[test]
fn batch_118_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-118");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (5, 5), "{report}");
    });
}
