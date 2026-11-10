//! Builds fixture batch 087 (collinear sketch lines, edge directions of offset faces, a tab across a corner, a cone
//! replaced by a cone, a held flange aligned outside, planes, curvatures and rays) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Faces and edges come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004", "c006"];

/// Known gaps (docs/FINDINGS.md): the tab across the box's corner closes the corner's rip in Onshape (std's derip),
/// where the engine finds no rip to close and leaves 0.02 mm slivers (c004, within 2e-6 of the volume); a sphere's
/// principal directions, all alike there, are Onshape's own choice (c008).
const KNOWN_GAPS: &[&str] = &["c004", "c008"];

#[test]
fn batch_087_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-087");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|gap| p.starts_with(gap))).collect();
        assert!(unexpected.is_empty(), "{report}");
    });
}
