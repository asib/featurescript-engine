//! Builds fixture batch 059 (intersection curve edges, offset curve directions, extruding in a sheet metal flat,
//! curvature fillets 0.87 to 0.97) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): none.
const KNOWN_GAPS: &[&str] = &[];

/// Onshape's offset of a circle across a plane is an approximation, 0.0015 mm shorter than the exact circle (c003).
/// The curvature fillets' areas and volumes stray from Onshape's quintics as in batch 053 (c010–c012).
const LOOSE: &[(&str, f64)] = &[("c003", 0.002), ("c010", 0.05), ("c011", 0.05), ("c012", 0.05)];

/// The values cases return (edges' ends, faces' surfaces) are there to study. Only the bodies are compared.
const CASES: &[&str] =
    &["c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008", "c009", "c010", "c011", "c012"];

#[test]
fn batch_059_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-059");
        let report = fs_fixtures::check_batch_bodies(&batch, None, LOOSE, CASES).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (13 - KNOWN_GAPS.len(), 13), "{report}");
    });
}
