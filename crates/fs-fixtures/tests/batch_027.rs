//! Builds fixture batch 027 (offset curves on curved faces, curves and sheets extended up to entities, fit splines
//! matching curvature, closed and elevated curves, a reapplied mirror) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/std-coverage.md): none. A closed spline raised in degree, which std hands back with
/// clamped knots but flagged periodic, Onshape keeps so; OCCT's curve can't be, so the engine keeps the flag on the
/// edge (c005).
const KNOWN_GAPS: &[&str] = &[];

#[test]
fn batch_027_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-027");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|gap| p.starts_with(gap))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 9), "{report}");
    });
}
