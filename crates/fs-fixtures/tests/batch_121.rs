//! Builds fixture batch 121 (boundary surfaces between lines with end conditions of other magnitudes and along
//! directions) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The curved surfaces' probes return Onshape's poles, four along the lines where the engine samples 33: only their
/// bodies are compared (their areas match to 1e-7).
const BODIES_ONLY: &[&str] = &["c003", "c004", "c005", "c006", "c007", "c008", "c009", "c010"];

#[test]
fn batch_121_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-121");
        let report = fs_fixtures::check_batch_bodies(&batch, None, &[], BODIES_ONLY).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (11, 11), "{report}");
    });
}
