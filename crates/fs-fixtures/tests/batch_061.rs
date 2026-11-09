//! Builds fixture batch 061 (extruding in a bent flat, extruded cylinders' coordinate systems about y and tilted
//! axes, crossing cylinders' intersection curves with the groups swapped, polygon lofts tangent to the end
//! profile, both, and off axis) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): a loft between triangles tangent
/// to the start, which Onshape fails (`LOFT_FAILED`) by a rule not found and the engine builds (c011).
const KNOWN_GAPS: &[&str] = &["c011"];

/// Intersection curves: Onshape's are nearly, not exactly, parametrised by length (c006, c007).
const LOOSE: &[(&str, f64)] = &[("c006", 0.025), ("c007", 0.025)];

/// Onshape lists a loft's sides and a sheet metal part's faces in an order of its own, and the swapped
/// intersection's edges not in its wire bodies' order: these are compared in any order.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c007", "c008", "c009", "c010"];

#[test]
fn batch_061_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-061");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], UNORDERED).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case)) && !p.starts_with("failed probe"))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (12 - KNOWN_GAPS.len() - 1, 12), "{report}");
    });
}
