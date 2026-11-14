//! Builds fixture batch 170 (probes: the poles of lofts from rectangles to circles) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// The faces' poles are Onshape's own representation (each arc a cubic of 16 spans, its knots doubled; the engine's
/// one span of degree 6 on the same surface): only the bodies are compared.
const BODIES_ONLY: &[&str] = &["c001", "c002", "c003"];

#[test]
fn batch_170_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-170");
        let report = fs_fixtures::check_batch_bodies(&batch, None, &[], BODIES_ONLY).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (4, 4), "{report}");
    });
}
