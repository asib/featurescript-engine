//! Builds fixture batch 014 (sheet metal hems, rips and flange options) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// The mitred flanges fail in Onshape and the engine alike (c006); neither can delete the plate the case leaves
/// behind, which keeps its default name, "Part 6", its place among the parts.
const KNOWN_GAPS: &[&str] = &[];

#[test]
fn batch_014_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-014");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (8, 8), "{report}");
    });
}
