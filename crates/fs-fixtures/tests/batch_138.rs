//! Builds fixture batch 138 (frames locked to curved faces along other paths) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// A square tube round the four edges a rectangle splits a cylinder's side along (c005): every segment's area, volume
/// and bounds are Onshape's (to 1e-6), and so are the areas its probe lists for the frame's faces, but in another
/// order (Onshape lists the straight segments' faces first); and Onshape's STEP of the two arc segments reads back
/// 0.03 mm³ (4e-3) small, as its own measure of them isn't.
const UNCOMPARABLE: &[&str] = &["c005 probe", "c005.2:", "c005.3:"];

#[test]
fn batch_138_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-138");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !UNCOMPARABLE.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (5, 6), "{report}");
    });
}
