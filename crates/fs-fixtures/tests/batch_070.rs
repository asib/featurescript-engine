//! Builds fixture batch 070 (curves of other degrees and rational ones extended, isoclines of half a tilted torus, a
//! draft both sides of a curved parting sheet, sweeps and a frame locked to curved faces, a flex PCB unfolded, a
//! conical shell recognized, a hole cut in a rolled wall's flat pattern) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// The hole Onshape wraps onto the rolled wall is its own fit of the curve: its faces' areas 0.009 mm² off, the
/// part's 0.025 (7e-6 of it).
const LOOSE: &[(&str, f64)] = &[("c012", 0.03)];
/// The isoclines' points are at Onshape's own parameters (its fit of each curve); their ends, lengths and boxes
/// match.
const BODIES_ONLY: &[&str] = &["c005"];
/// The parts' faces come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c007", "c008", "c009", "c012"];

#[test]
fn batch_070_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-070");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, BODIES_ONLY, UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (13, 13), "{report}");
    });
}
