//! Builds fixture batch 141 (everyday parts: a hole grid, a mirrored gusset, a lofted transition, a sheet metal
//! channel, a split block, an intersection, a thickened loft, countersinks) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): a square lofted to a circle, shelled
/// open at both ends (c003), its sides meeting tangent at the circle, where OCCT's offsets fail.
const KNOWN_GAPS: &[&str] = &["c003", "failed probe"];

/// The channel's and the split block's body counts (c004, c005) take in c003's sketch sheets, which Onshape keeps and
/// the engine, failing c003, removes: their parts are compared, not their counts.
const BODIES_ONLY: &[&str] = &["c004", "c005"];

#[test]
fn batch_141_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-141");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], BODIES_ONLY, &[]).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (7, 9), "{report}");
    });
}
