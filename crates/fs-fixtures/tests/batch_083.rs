//! Builds fixture batch 083 (face blends with two solutions and a help point; hold lines and a cliff edge inside a
//! side) on the engine and compares it with Onshape's. Onshape offered no STEP export (the Part Studio held no
//! parts), so its onshape.step is empty and the probes carry the comparison.

use std::path::PathBuf;

/// Faces come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c002"];

/// With a help point near one of the two blends a floor makes with a Π-shaped sheet's walls, Onshape fails
/// (`FACE_BLEND_FAILED`, c001), and so does the engine; without one both blend at both walls.
const KNOWN_GAPS: &[&str] = &[];

#[test]
fn batch_083_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-083");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|gap| p.starts_with(gap))).collect();
        assert!(unexpected.is_empty(), "{report}");
    });
}
