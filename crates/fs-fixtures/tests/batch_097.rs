//! Builds fixture batch 097 (more sketch text: a phrase, a kerning pair, a composite glyph, boxes elsewhere,
//! construction text, descenders) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Onshape lists a sketch's regions in an order of its own once there are several glyphs.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c007"];

/// Extruded, the Béziers nearest circular arcs make cylinders in Onshape, its kernel's simplification
/// ([`fs_fixtures::allowing_text_simplification`]); the probe lines, with each body's volume and area, match.
#[test]
fn batch_097_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-097");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        let problems = fs_fixtures::allowing_text_simplification(report.problems());
        assert!(problems.is_empty(), "{problems:?}\n{report}");
        assert_eq!(report.probes_matched, (8, 8), "{report}");
    });
}
