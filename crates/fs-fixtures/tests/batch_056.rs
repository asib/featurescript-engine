//! Builds fixture batch 056 (loft end conditions off axis and along directions, face blends, curvature fillets 0.72
//! to 0.84, sphere bands) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): spheres' bands extended round
/// (c019–c021).
const KNOWN_GAPS: &[&str] = &["c019", "c020", "c021"];

/// A loft from a square to one off its axis: Onshape's sides are fits that rise and fall along each edge about the
/// exact construction's (its volume 0.05 mm³ over). Curvature fillets: Onshape's own areas of its quintic sections
/// stray from them (as in batch 053), by up to 0.012 mm².
const LOOSE: &[(&str, f64)] =
    &[("c001", 0.15), ("c014", 0.05), ("c015", 0.05), ("c016", 0.05), ("c017", 0.05), ("c018", 0.05)];

/// Every case returns its faces' surfaces or points along its edges, to study. Only the bodies are compared, but for
/// the loft tangent to a square, whose sides the engine builds as Onshape does (batch 060), compared in any order.
const CASES: &[&str] = &[
    "c001", "c002", "c004", "c005", "c006", "c007", "c008", "c009", "c010", "c011", "c012", "c013", "c014",
    "c015", "c016", "c017", "c018", "c019", "c020", "c021",
];

#[test]
fn batch_056_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-056");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, CASES, &["c003"]).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case)))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (22 - KNOWN_GAPS.len(), 22), "{report}");
    });
}
