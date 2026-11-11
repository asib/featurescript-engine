//! Builds fixture batch 108 (lofts and face replacements Onshape refuses, varied) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Cases the engine and Onshape don't agree on (see docs/std-coverage.md): a loft between arcs extended 5 mm at
/// every edge, which the engine can't extend (c011: failing in both, the engine before the replacement).
const KNOWN_GAPS: &[&str] = &["c011"];

/// A loft normal to a profile: Onshape blends the derivative along each edge by a rule not found (as batch 063's).
const LOOSE: &[(&str, f64)] = &[("c007", 0.02)];

#[test]
fn batch_108_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-108");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], &[]).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (13 - KNOWN_GAPS.len(), 13), "{report}");
    });
}
