//! Builds fixture batch 006 (holes, ribs, modify fillet, body draft, face patterns, face splits, tangent planes,
//! composite parts, thin revolve and sweep, and revolving up to a face) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/std-coverage.md): a plane tangent to a cylinder, placed from the cylinder's surface
/// origin, which Onshape puts at the far end of an extruded cylinder (c016).
const KNOWN_GAPS: &[&str] = &["c016"];

#[test]
fn batch_006_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-006");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (22 - KNOWN_GAPS.len(), 22), "{report}");
    });
}
