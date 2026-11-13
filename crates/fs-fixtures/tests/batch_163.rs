//! Builds fixture batch 163 (probes of the extra term in a boundary surface's derivative at magnitudes other than 1)
//! on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Every case differs (see docs/FINDINGS.md): Onshape builds these surfaces pole by pole on the knots across its
/// profiles (an arc its own least-squares cubic fit), and at magnitudes other than 1 fits the leaving derivative at
/// those knots with a pull of 0.04 toward the magnitude-1 one; the engine builds them from sampled rows on its own
/// knots, and refuses magnitudes other than 1 between curved profiles.
const KNOWN_GAPS: &[&str] = &["c0", "failed probe"];

#[test]
fn batch_163_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-163");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (0, 10), "{report}");
    });
}
