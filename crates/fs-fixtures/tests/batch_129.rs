//! Builds fixture batch 129 (where equal chamfers of curved edges meet their faces) on the engine and compares it
//! with Onshape's.

use std::path::PathBuf;

/// Chamfers along intersection curves (c001–c004): Onshape's surface between the fillet's edges is its own fit, its
/// face areas up to 0.02 mm² off the engine's (volumes within 0.03 mm³).
const LOOSE: &[(&str, f64)] = &[("c001", 0.05), ("c002", 0.05), ("c003", 0.05), ("c004", 0.05)];

/// The chamfers' edges' points are taken by parameter along each curve, which each kernel lays out its own way (and
/// along circles starts elsewhere round, in another order): the values are there to study, so only the bodies are
/// compared (they match; the edges' points agree to Onshape's fits where measured, docs/FINDINGS.md).
const BODIES_ONLY: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006", "c007"];

/// The tee's 2 mm chamfer and Onshape's lie too close for OCCT's boolean between them (it reports nonsense).
const UNCOMPARABLE: &[&str] = &["c002.1: the shapes differ"];

#[test]
fn batch_129_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-129");
        let report = fs_fixtures::check_batch_bodies(&batch, None, LOOSE, BODIES_ONLY).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !UNCOMPARABLE.iter().any(|u| p.starts_with(u))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (8, 8), "{report}");
    });
}
