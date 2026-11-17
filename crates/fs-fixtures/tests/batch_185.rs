//! Builds fixture batch 185 (sketch text in the other fonts) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Onshape lists the regions in its own order.
const UNORDERED: &[&str] = &[
    "c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008", "c009", "c010", "c011", "c012", "c013", "c014",
    "c015", "c016", "c017", "c018", "c019", "c020", "c021", "c022",
];

#[test]
fn batch_185_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-185");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (23, 23), "{report}");
    });
}
