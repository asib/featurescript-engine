//! Builds fixture batch 005 (curve features, surface features and mate connectors) on the engine and compares it
//! with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/std-coverage.md): none.
const KNOWN_GAPS: &[&str] = &[];

/// A boundary surface between two arcs: Onshape rules between its own eight-span fits of the arcs, 0.003 mm² under
/// the engine's exact ruled surface (c019; batch 062 c001 shows its poles).
const LOOSE: &[(&str, f64)] = &[("c019", 0.005)];

#[test]
fn batch_005_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-005");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (30 - KNOWN_GAPS.len(), 30), "{report}");
    });
}
