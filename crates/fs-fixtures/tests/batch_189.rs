//! Builds fixture batch 189 (more curved faces replaced) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine doesn't build as Onshape does (see docs/FINDINGS.md): a loft's side up to a plane (c004), which
/// Onshape builds, its lofted neighbours extended, and the engine refuses; the failed cases' line shows it.
const KNOWN_GAPS: &[&str] = &["c004", "failed probe"];

/// Onshape lists the faces in its own order.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004", "c005"];

#[test]
fn batch_189_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-189");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (4, 6), "{report}");
    });
}
