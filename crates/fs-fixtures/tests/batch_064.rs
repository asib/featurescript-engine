//! Builds fixture batch 064 (curvature fillets from magnitude 0.975 to 0.9975, flat patterns of more bent plates)
//! on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): curvature fillets past magnitude
/// 0.979, whose sections follow constraints Onshape's construction switches between by a rule not found (c003–c006).
/// c008's point lies on no face of the flange, so it fails in both; its plate is left, unnamed, which Onshape's model
/// keeps as Part 8 and the engine's leaves out.
const KNOWN_GAPS: &[&str] = &["c003", "c004", "c005", "c006", "Part 8"];

/// Curvature fillets of magnitude 0.975 and 0.9775: Onshape's third pole there still falls as the engine's, but
/// its β₃ stays near 7.3e-5 where the engine's formula gives 1.7e-4 and 2.9e-5, so their areas are within 0.006
/// mm² (c001, c002).
const LOOSE: &[(&str, f64)] = &[("c001", 0.01), ("c002", 0.01)];

/// Only the fillets' bodies are compared (their sections differ slightly); the sheet metal parts' faces are
/// compared in any order.
const BODIES_ONLY: &[&str] = &["c001", "c002"];
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
        assert_eq!(report.probes_matched, (11 - 4 - 1, 11), "{report}");
    });
}
