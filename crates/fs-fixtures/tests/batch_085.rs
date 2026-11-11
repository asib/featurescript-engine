//! Builds fixture batch 085 (a cone side replaced by a cylinder, isoparametric curves and parameters of full tori, a
//! variable fillet at a corner) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// The corner fillet's surfaces are Onshape's cubic fits: volume to 5.4e-6, faces to 0.014 mm².
const LOOSE: &[(&str, f64)] = &[("c005", 0.1)];

/// Faces come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c001", "c005"];

/// Cases the engine and Onshape don't agree on (see docs/std-coverage.md): none.
const KNOWN_GAPS: &[&str] = &[];

#[test]
fn batch_085_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-085");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|gap| p.starts_with(gap))).collect();
        assert!(unexpected.is_empty(), "{report}");
    });
}
