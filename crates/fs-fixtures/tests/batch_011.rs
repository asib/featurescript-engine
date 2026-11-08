//! Builds fixture batch 011 (edit curve, path deviation, constrained surfaces, routing curves and their tables,
//! computed mass, decals, form tags, cut lists and hole tables) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/std-coverage.md): the constrained surface, which Onshape fits its own way (c006).
const KNOWN_GAPS: &[&str] = &["c006"];

/// Approximations, compared to within their own tolerance (mm): a line and an arc fitted with one cubic (c003).
const LOOSE: &[(&str, f64)] = &[("c003", 0.01)];

#[test]
fn batch_011_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-011");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (15 - KNOWN_GAPS.len(), 15), "{report}");
    });
}
