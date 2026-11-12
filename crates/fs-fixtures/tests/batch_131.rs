//! Builds fixture batch 131 (sheet metal: bend alignments, slanted bends, jogs, corner reliefs, rip joints) on the
//! engine and compares it with Onshape's.

use std::path::PathBuf;

/// A round corner relief and a closed corner (c014, c015): Onshape's relieved bends are B-spline faces of its own,
/// their areas and volumes within 0.01 (as in batches 013 and 016).
const LOOSE: &[(&str, f64)] = &[("c014", 0.01), ("c015", 0.01)];

#[test]
fn batch_131_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-131");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (20, 20), "{report}");
    });
}
