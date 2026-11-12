//! Builds fixture batch 130 (chamfers where the two rules differ: corners other than square, uneven chamfers of
//! curved edges) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Uneven chamfers along an intersection curve (c004, c005): Onshape's surface between the feet is its own fit, its
/// areas within 0.014 mm² and volumes within 0.02 mm³ of the engine's.
const LOOSE: &[(&str, f64)] = &[("c004", 0.05), ("c005", 0.05)];

#[test]
fn batch_130_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-130");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (7, 7), "{report}");
    });
}
