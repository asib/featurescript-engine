//! Builds fixture batch 067 (offsets down a cone and across a torus, a curving extension up to a circle, a ruled
//! surface normal to a cone, a face replaced up to a lofted sheet) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine and Onshape don't agree on (see docs/std-coverage.md): none.
const KNOWN_GAPS: &[&str] = &[];

/// The torus's offset: the engine's closed curve is the same parallel (radius 7.917666) but starts where its
/// samples do, not where Onshape's does (at 7.5° round, c002). The ruled band: Onshape's surface is its own fit of
/// the circles, the area 4.5e-4 mm² off the engine's (c004).
const LOOSE: &[(&str, f64)] = &[("c004", 0.002)];
const BODIES_ONLY: &[&str] = &["c002", "c004"];

#[test]
fn batch_067_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-067");
        let report = fs_fixtures::check_batch_bodies(&batch, None, LOOSE, BODIES_ONLY).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case)))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (6 - KNOWN_GAPS.len(), 6), "{report}");
    });
}
