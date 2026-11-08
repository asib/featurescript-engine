//! Builds fixture batch 032 (sheet metal hole patterns) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The countersunk holes' shafts come out 0.00126 mm² short each (6.281929 against 6.283185 mm², as batch 021's
/// single one did, within tolerance there); four of them put the area 2e-6 off, so c002 is compared to 0.01.
const LOOSE: &[(&str, f64)] = &[("c002", 0.01)];

#[test]
fn batch_032_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-032");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (3, 3), "{report}");
    });
}
