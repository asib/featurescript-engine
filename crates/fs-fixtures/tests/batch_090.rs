//! Builds fixture batch 090 (mate connectors by axis points, loop centres, a virtual sharp, a cone tip and a sphere
//! centre; an aligned section of a composite) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// After the aligned section of the composite (c010) the cut pieces of the default planes' rotated copies stay
/// construction planes (a subtraction keeps them so, batch 092).
const KNOWN_GAPS: &[&str] = &[];

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
