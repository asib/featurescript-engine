//! Builds fixture batch 198 (sketch circular pattern and pierce constraints) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Cases the engine doesn't solve as Onshape does (see docs/FINDINGS.md): an open circular pattern (c003), whose
/// instances Onshape spaces evenly by a step of its own (86.83° for a given 270°), not the angle given.
const KNOWN_GAPS: &[&str] = &["c003"];

#[test]
fn batch_198_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-198");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (8, 9), "{report}");
    });
}
