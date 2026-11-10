//! Builds fixture batch 077 (sheet metal loft matches, a slanted bend's end faces, a block's top replaced by a
//! sphere) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The slanted bend's end faces lie on the engine's ruled surface carried past the bend to cut it, so their poles
/// (and points along their curved edges) aren't Onshape's; the part itself matches.
const BODIES_ONLY: &[&str] = &["c004"];

/// Faces come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c005"];

#[test]
fn batch_077_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-077");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], BODIES_ONLY, UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (6, 6), "{report}");
    });
}
