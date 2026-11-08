//! Builds fixture batch 038 (tags for PCB holes and standard content; curves extended at their start, trimmed by
//! planes and lines; elliptical arcs extended) on the engine and compares it with Onshape's.

use std::path::PathBuf;

#[test]
fn batch_038_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-038");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (15, 15), "{report}");
    });
}
