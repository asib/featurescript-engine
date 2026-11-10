//! Builds fixture batch 076 (a sweep and a frame locked to a sphere, a quartic extended, offset curves extended and
//! split, a thin loft trimmed to its profiles) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Faces come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c001", "c002", "c007", "c008"];

#[test]
fn batch_076_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-076");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 9), "{report}");
    });
}
