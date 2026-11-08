//! Builds fixture batch 020 (replace face with a curved template, extend up to, wraps, measured diameter, curves,
//! mate connector normal, sheet metal corner breaks) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/std-coverage.md): Onshape makes no curve offsetting a cylinder's top edge across its
/// top, for no reason std shows (c008).
const KNOWN_GAPS: &[&str] = &["c008"];

/// Onshape's conic corner break is a B-spline whose shoulder sits at 0.69971 rather than the 0.7 asked for (an
/// approximation): areas and volumes are compared to 0.01 (2.4e-6 of them), surfaces to 5 µm (c012).
const LOOSE: &[(&str, f64)] = &[("c012", 0.01)];

#[test]
fn batch_020_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-020");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (12, 13), "{report}");
    });
}
