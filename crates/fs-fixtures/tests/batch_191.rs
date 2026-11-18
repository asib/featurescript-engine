//! Builds fixture batch 191 (periodicity and iso curves of swept and revolved tori) on the engine and compares it
//! with Onshape's.

use std::path::PathBuf;

/// Onshape lists the faces in its own order.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004"];

#[test]
fn batch_191_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-191");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (5, 5), "{report}");
    });
}
