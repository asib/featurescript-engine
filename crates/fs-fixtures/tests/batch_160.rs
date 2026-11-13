//! Builds fixture batch 160 (probes: undimensioned circles between lines at other angles; boundary surfaces to an arc
//! at other magnitudes) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/FINDINGS.md): boundary surfaces from a sheet's edge to an arc at magnitudes 2 and 0.5
/// (c006, c007), which the engine refuses: Onshape's curve across is the magnitude-1 one's height, its middle poles
/// shifted along the derivative by 0.11 times (1 - magnitude), a term no rule found gives. The failed cases' line
/// shows them.
const KNOWN_GAPS: &[&str] = &["c006", "c007", "failed probe"];

#[test]
fn batch_160_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-160");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (5, 8), "{report}");
    });
}
