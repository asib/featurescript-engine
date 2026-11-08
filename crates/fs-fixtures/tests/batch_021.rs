//! Builds fixture batch 021 (offset booleans, holes from planes, up-to entities, curve trims, sweep profile control,
//! ribs, gussets, tables, sheet metal holes and hems) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/std-coverage.md): hems meeting at a closed corner, where Onshape leaves the plate
/// whole and the engine cuts its corner (c012).
const KNOWN_GAPS: &[&str] = &["c012", "engine STEP: c012"];

/// A rectangle swept along an arc keeping its orientation: the engine's surfaces are exact (742.8419 mm², as by
/// hand), Onshape's B-spline sides 1.6e-5 short of that; compared to 0.05 (c006).
const LOOSE: &[(&str, f64)] = &[("c006", 0.05)];

#[test]
fn batch_021_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-021");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (12, 13), "{report}");
    });
}
