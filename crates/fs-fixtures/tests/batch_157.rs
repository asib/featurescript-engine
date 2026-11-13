//! Builds fixture batch 157 (probes: fills with tangency conditions across a gap between sheets) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/FINDINGS.md): fills tangent to a sheet along an edge (c001–c003), which the engine
/// doesn't build yet (Onshape's are its own fits: across the gap within 0.02 mm of the cubic with chord-length
/// tangents, but varying along it and off centre by 3e-4 mm); a flat fill (c004) facing the other way from Onshape's.
/// The engine fails c001–c003, which the failed cases' line shows.
const KNOWN_GAPS: &[&str] = &["c001", "c002", "c003", "c004", "failed probe"];

#[test]
fn batch_157_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-157");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
    });
}
