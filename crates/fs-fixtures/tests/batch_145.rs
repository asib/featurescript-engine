//! Builds fixture batch 145 (everyday parts, a sixth round: a nut, a bolt, a hinge leaf, a spoked wheel, a bottle, a
//! ribbed bracket, a box lip, a notched knob) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The bottle (c005), a loft through three circles shelled: Onshape's sides are its own fits, its area 0.01 mm² over
/// and its volume 0.076 mm³ (1e-5) under the engine's. The engine's own volume is only good to about that: its
/// inner wall is the loft's offset, over which OCCT's integration moves by up to 0.15 mm³ with how finely the face
/// is split (7413.37, 7413.22, 7413.08 and 7413.15 split 2, 4, 8 and 12 ways each way; Onshape's is 7413.14).
const LOOSE: &[(&str, f64)] = &[("c005", 0.1)];

#[test]
fn batch_145_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-145");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 9), "{report}");
    });
}
