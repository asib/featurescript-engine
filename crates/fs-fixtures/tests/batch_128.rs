//! Builds fixture batch 128 (joints of curved faces: tees, a capsule, a window, a boss on a pipe, crossing pipes) on
//! the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): the tee shelled open at its ends,
/// which OCCT's offsets make without the branch's bore (c003: refused rather than built wrong).
const KNOWN_GAPS: &[&str] = &["c003"];

/// The crossing pipes' fillets and Onshape's lie too close for OCCT's boolean between them (it reports nonsense);
/// their volumes, areas, boxes and topology match.
const UNCOMPARABLE: &[&str] = &["c007.1: the shapes differ"];

/// The tee's chamfer (c002): Onshape's surface between the fillet's edges is its own fit, 0.012 mm² off.
const LOOSE: &[(&str, f64)] = &[("c002", 0.05)];

#[test]
fn batch_128_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-128");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| {
                !KNOWN_GAPS.iter().chain(UNCOMPARABLE).any(|case| p.starts_with(case)) && !p.starts_with("failed probe")
            })
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        // Every probe line but the gaps' and the failed-cases line (c003 fails in the engine alone).
        assert_eq!(report.probes_matched, (8 - KNOWN_GAPS.len() - 1, 8), "{report}");
    });
}
