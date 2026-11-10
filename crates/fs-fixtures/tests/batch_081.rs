//! Builds fixture batch 081 (how far a slanted sheet metal bend's far end tilts: a bend radius of 2, 1 mm thick,
//! shifted 5 and 20 mm, the material on the other side) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The end faces lie on the engine's ruled surfaces carried past the bend to cut it, so their poles aren't Onshape's;
/// the parts match.
const BODIES_ONLY: &[&str] = &["c001", "c002", "c003", "c004", "c005"];

#[test]
fn batch_081_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-081");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], BODIES_ONLY, &[]).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (6, 6), "{report}");
    });
}
