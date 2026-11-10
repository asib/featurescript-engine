//! Builds fixture batch 090 (mate connectors by axis points, loop centres, a virtual sharp, a cone tip and a sphere
//! centre; an aligned section of a composite) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Known gap (docs/FINDINGS.md): after the aligned section of the composite (c010) Onshape leaves two of the section's
/// planes as construction planes 6 inches across; the engine leaves them as sheets 150 mm across. The composite (its
/// bodies and shape) and the section's parts match.
const KNOWN_GAPS: &[&str] = &["c010.4", "c010.5", "c010 probe"];

#[test]
fn batch_090_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-090");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], &[]).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|gap| p.starts_with(gap))).collect();
        assert!(unexpected.is_empty(), "{report}");
    });
}
