//! Builds fixture batch 002 (revolve, primitives, booleans, transforms, patterns, splits, planes, points and
//! variables) on the engine and compares every part with Onshape's STEP and every probe line with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_002_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-002");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.parts.len(), 139, "{report}");
        assert_eq!(report.probes_matched, (84, 84), "{report}");
    });
}
