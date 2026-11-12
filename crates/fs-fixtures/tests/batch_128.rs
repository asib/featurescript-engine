//! Builds fixture batch 128 (joints of curved faces: tees, a capsule, a window, a boss on a pipe, crossing pipes) on
//! the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): the tee's joint chamfered, its
/// 1 mm measured otherwise (c002: 0.85 mm² apart); the tee shelled open at its ends, which OCCT's offsets make
/// without the branch's bore (c003: refused rather than built wrong); the crossing pipes' joints filleted, OCCT's
/// fillet three faces each where Onshape's is one (c007: areas 0.01 mm² apart).
const KNOWN_GAPS: &[&str] = &["c002", "c003", "c007"];

#[test]
fn batch_128_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-128");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case)) && !p.starts_with("failed probe"))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        // Every probe line but the gaps' and the failed-cases line (c003 fails in the engine alone).
        assert_eq!(report.probes_matched, (8 - KNOWN_GAPS.len() - 1, 8), "{report}");
    });
}
