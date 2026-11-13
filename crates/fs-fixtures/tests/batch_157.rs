//! Builds fixture batch 157 (probes: fills with tangency conditions across a gap between sheets) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/FINDINGS.md): fills tangent to the level sheet along its edge (c001, c002), which the
/// engine doesn't build yet (Onshape's are its own fits: across the gap within 0.02 mm of the cubic with chord-length
/// tangents, but varying along it and off centre by 3e-4 mm). The engine fails them, which the failed cases' line
/// shows. The flat fill, tangent to the rising sheet it already continues (c003) or not (c004), matches.
const KNOWN_GAPS: &[&str] = &["c001", "c002", "failed probe"];

#[test]
fn batch_157_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-157");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (2, 5), "{report}");
    });
}
