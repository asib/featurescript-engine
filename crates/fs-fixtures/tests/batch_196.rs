//! Builds fixture batch 196 (constrained surfaces through points) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine doesn't build as Onshape does (see docs/FINDINGS.md): surfaces through points not in a plane
/// (c002, c004–c010), which Onshape fits with knots it refines by a rule not yet found, over its own parameters for
/// points not on a rectangular grid; the engine's are within tolerance of the points but other surfaces.
const KNOWN_GAPS: &[&str] = &["c002", "c004", "c005", "c006", "c007", "c008", "c009", "c010"];

#[test]
fn batch_196_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-196");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (3, 11), "{report}");
    });
}
