//! Builds fixture batch 018 (options of covered features: split by a sheet, move face up to, curves, surfaces,
//! mate connectors, sheet metal holes and unfolds) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// c013 fails in Onshape and the engine alike (its hold face pick misses). Onshape leaves the case's three sheet
/// metal parts, which can't be deleted (a rolled wall between two flat ones, ripped apart), and names them
/// "Part 13" to "Part 15", their places among the parts; the engine leaves four, "Part 12" and "Part 14" to "Part 16".
const KNOWN_GAPS: &[&str] = &["Part 12", "Part 13", "Part 14", "Part 15", "Part 16"];

#[test]
fn batch_018_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-018");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (14, 14), "{report}");
    });
}
