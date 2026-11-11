//! Builds fixture batch 064 (curvature fillets from magnitude 0.975 to 0.9975, flat patterns of more bent plates)
//! on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): a curvature fillet of magnitude
/// 0.9975, its section within micrometres of the corner, too thin a tool for the engine's booleans (c006). c008's
/// point lies on no face of the flange, so it fails in both; its plate is left, unnamed: Part 8 in Onshape's model,
/// its place among the parts, Part 7 in the engine's, without c006's.
const KNOWN_GAPS: &[&str] = &["c006", "Part 7", "Part 8"];

/// Curvature fillets of magnitude 0.975 to 0.9925, their sections Onshape's own, sampled: its areas and volumes of
/// its quintics stray, as in batch 053 (within 0.02).
const LOOSE: &[(&str, f64)] = &[("c001", 0.05), ("c002", 0.05), ("c003", 0.05), ("c004", 0.05), ("c005", 0.05)];

/// Only the fillets' bodies are compared (their sections differ slightly); the sheet metal parts' faces are
/// compared in any order.
const BODIES_ONLY: &[&str] = &["c001", "c002", "c003", "c004", "c005"];
const UNORDERED: &[&str] = &["c007", "c009", "c010"];

#[test]
fn batch_064_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-064");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, BODIES_ONLY, UNORDERED).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case)) && !p.starts_with("failed probe"))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (11 - 1 - 1, 11), "{report}");
    });
}
