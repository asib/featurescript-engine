//! Builds fixture batch 188 (lofts of other profiles) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine doesn't build as Onshape does yet (see docs/FINDINGS.md): a rounded rectangle to a circle (c001,
/// c002: Onshape's slopes at the nodes not the geometric means, its circle split at every vertex); a D (c003: a face
/// for each edge, the half circle's a cylinder); an ellipse to an ellipse, a slot to one (c004, c005); a closed fit
/// spline to a circle (c006); an ellipse and a slot connected twice (c008, c009).
const KNOWN_GAPS: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006", "c008", "c009"];

/// Onshape lists the faces in its own order.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008", "c009"];

#[test]
fn batch_188_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-188");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (2, 10), "{report}");
    });
}
