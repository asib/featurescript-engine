//! Builds fixture batch 093 (splits by self shadow: blocks, a cylinder, a sphere and a cone frustum along their
//! silhouettes; a tower's, a boss's and a dome's shadows; bodies shadowing each other) on the engine and compares it
//! with Onshape's.

use std::path::PathBuf;

/// The shadows cast on the plates by the boss's top and the dome's silhouette are Onshape's approximations, the
/// regions they bound 0.0078 mm² (of 150) and 0.028 mm² (of 65.06) bigger than the exact ones, their edges up to
/// 0.0011 mm longer (c009, c010).
const LOOSE: &[(&str, f64)] = &[("c009", 0.01), ("c010", 0.03)];

#[test]
fn batch_093_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-093");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], &[]).unwrap();
        assert!(report.problems().is_empty(), "{report}");
    });
}
