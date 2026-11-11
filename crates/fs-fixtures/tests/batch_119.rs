//! Builds fixture batch 119 (sweeps locked to a sphere along great circles, an extruded elliptical sheet extended) on
//! the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The sweeps' faces come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c001", "c002"];

/// The extended sheets' new straight edge isn't a line to Onshape's `qGeometry` (its extension makes it otherwise; the
/// engine's is a line), so their curve lists differ by it: only the bodies are compared (c003, c004).
const BODIES_ONLY: &[&str] = &["c003", "c004"];

#[test]
fn batch_119_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-119");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], BODIES_ONLY, UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (5, 5), "{report}");
    });
}
