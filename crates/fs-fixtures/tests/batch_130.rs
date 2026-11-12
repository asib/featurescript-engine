//! Builds fixture batch 130 (chamfers where the two rules differ: corners other than square, uneven chamfers of
//! curved edges) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): uneven chamfers along an intersection
/// curve, a tee's joint with offsets 1 and 2 (c004) and 1 mm at 30° (c005), which Onshape doesn't make as OCCT does
/// (7.7 and 3.3 mm³ apart) and whose rule isn't found.
const KNOWN_GAPS: &[&str] = &["c004", "c005"];

#[test]
fn batch_130_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-130");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (7 - KNOWN_GAPS.len(), 7), "{report}");
    });
}
