//! Builds fixture batch 137 (sweeps locked to curved faces along other paths) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// The square round the curve on a cylinder (c004): the engine's ring is exact (21.040594 mm³ against the path's
/// length, 21.040590 mm, times the square's area, by Pappus); Onshape's is its own approximation, 7.2e-4 over.
const LOOSE: &[(&str, f64)] = &[("c004", 0.05)];


#[test]
fn batch_137_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-137");
        // That ring and Onshape's lie too close for OCCT's boolean between them (it reports nonsense, slowly):
        // compared by their volumes, areas, boxes and topology.
        fs_fixtures::skip_shape_difference(&["c004.2"]);
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (6, 6), "{report}");
    });
}
