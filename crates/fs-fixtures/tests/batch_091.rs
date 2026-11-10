//! Builds fixture batch 091 (sketch text) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The text matches exactly (FreeType's unhinted outlines at 100 pixels to the em, docs/FINDINGS.md). Extruded, two
/// of "Ab"'s b's Béziers make cylinders in Onshape, its kernel's simplification
/// ([`fs_fixtures::allowing_text_simplification`]); its probe line, with each body's volume and area, matches.
#[test]
fn batch_091_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-091");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], &[]).unwrap();
        let problems = fs_fixtures::allowing_text_simplification(report.problems());
        assert!(problems.is_empty(), "{problems:?}\n{report}");
        assert_eq!(report.probes_matched, (6, 6), "{report}");
    });
}
