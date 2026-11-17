//! Builds fixture batch 187 (more lofts with connections; a slot and an ellipse lofted to a circle) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine doesn't build as Onshape does yet (see docs/FINDINGS.md): a slot and an ellipse lofted to a
/// circle, with a connection (c010, c011, which the engine refuses) and without (c012, c013): Onshape's side is one
/// periodic face of cubic Hermite spans along the same correspondence as connected lofts', where the engine's are a
/// face for each of the slot's edges and an exact ruled surface.
const KNOWN_GAPS: &[&str] = &["c010", "c011", "c012", "c013", "failed probe"];

/// Onshape's circular ends measured on other approximations of the arcs than its sides' edges (as in batch 180).
const LOOSE: &[(&str, f64)] = &[("c005", 0.002), ("c006", 0.002), ("c007", 0.002), ("c008", 0.05), ("c009", 0.002)];

/// Onshape lists the faces in its own order.
const UNORDERED: &[&str] =
    &["c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008", "c009", "c010", "c011", "c012", "c013"];

#[test]
fn batch_187_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-187");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 14), "{report}");
    });
}
