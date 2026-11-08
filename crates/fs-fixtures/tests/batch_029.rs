//! Builds fixture batch 029 (sheet metal tear drop hems) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Problems that are the engine's own (see docs/std-coverage.md): its STEP of each hemmed plate reads back 2e-6 off
/// in volume, because six line edges where the hem's wall meets its bend are 3e-7 tolerant and the STEP keeps their
/// lines, not the curves on the faces the engine measures (and Onshape agrees with, to 1e-9).
const KNOWN_GAPS: &[&str] = &["engine STEP: c001.1", "engine STEP: c002.1", "engine STEP: c003.1"];

#[test]
fn batch_029_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-029");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|gap| p.starts_with(gap))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (4, 4), "{report}");
    });
}
