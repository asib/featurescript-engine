//! Builds fixture batch 099 (a sphere's principal directions) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Known gap (docs/FINDINGS.md): on a sphere every direction is principal, and Onshape's least is n × x, n × y or
/// n × z (n the normal) by a choice of its kernel's own that no rule tried reproduces; only the bodies are compared.
const BODIES_ONLY: &[&str] = &["c001", "c002", "c003"];

#[test]
fn batch_099_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-099");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], BODIES_ONLY, &[]).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (4, 4), "{report}");
    });
}
