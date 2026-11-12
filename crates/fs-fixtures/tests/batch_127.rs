//! Builds fixture batch 127 (features in sequence: shells with fillets and holes, ribs, mirrors and splits of
//! shells, thickened fillets) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The united balls' spheres from two kernels, a hair apart, defeat OCCT's boolean between them (it reports the
/// whole volume); Onshape's volume, area and box of them match.
const UNCOMPARABLE: &[&str] = &["c007.1: the shapes differ"];

#[test]
fn batch_127_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-127");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !UNCOMPARABLE.iter().any(|u| p.starts_with(u))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 9), "{report}");
    });
}
