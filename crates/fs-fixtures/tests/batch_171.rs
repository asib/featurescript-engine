//! Builds fixture batch 171 (probes: boundary surfaces leaving short arcs normal to their planes) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/FINDINGS.md): Onshape's derivative leaving an arc normal to its plane matches the
/// chord less its part across the arc at the ends and where symmetry forces it, but along the arc in between it is
/// 18 to 60% larger than that, by a rule not found; the engine's surfaces (from sampled rows) are within 0.04 mm² of
/// Onshape's areas; at magnitude 2 (c004), its leaving derivative fitted as Onshape fits other magnitudes (batches
/// 162, 163), within 0.04 mm² too.
const KNOWN_GAPS: &[&str] = &["c001", "c002", "c003", "c004"];

#[test]
fn batch_171_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-171");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (1, 5), "{report}");
    });
}
