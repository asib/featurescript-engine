//! Builds fixture batch 153 (fills of non-planar loops: a pillow, a saddle of arcs and lines, a twisted loop, a
//! triangle of splines) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The pillow (c001): the Coons patch of four arcs, Onshape's from its own fits of them, its area 0.0033 mm² under the
/// engine's (the exact patch, by angle along each arc, is Onshape's to 2e-4 mm²).
const LOOSE: &[(&str, f64)] = &[("c001", 0.01)];

/// Cases that differ (see docs/FINDINGS.md): a three-sided loop (c004), which Onshape fills with a surface whose rule
/// isn't found (not a Coons patch with a side collapsed to any corner, 405 mm² to its 389.91); the engine's is
/// OCCT's (393.17 mm²).
const KNOWN_GAPS: &[&str] = &["c004"];

#[test]
fn batch_153_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-153");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (4, 5), "{report}");
    });
}
