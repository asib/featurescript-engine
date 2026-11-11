//! Builds fixture batch 116 (batch 037's sections again, the aligned section's leftover sheets' curves probed) on
//! the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The aligned section's leftover sheet is bounded by a curve that is a cubic in Onshape, through the ends of the
/// engine's quartic with its end derivatives (c004): only the bodies are compared.
const BODIES_ONLY: &[&str] = &["c004"];

#[test]
fn batch_116_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-116");
        let report = fs_fixtures::check_batch_bodies(&batch, None, &[], BODIES_ONLY).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (5, 5), "{report}");
    });
}
