//! Builds fixture batch 062 (boundary surfaces, a non-planar fill, flats of a channel and across a bend, a
//! three-profile tangent loft) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): a boundary surface tangent to its
/// profile, which the case leaves without `uReference` so std finds no `createProfileConditions` in either (c003);
/// a circle across a bend cut in the flat, which Onshape wraps onto the bend (c006; the engine fails it, leaving its
/// part as Part 3).
const KNOWN_GAPS: &[&str] = &["c003", "c006", "Part 3"];

/// Boundary surfaces between arcs: Onshape rules between its own eight-span fits of the arcs, 0.003 mm² off the
/// engine's exact arcs (c001, c002). The three-profile loft's knot across is 5e-5 from Onshape's, by a rule not
/// found: its poles are within 0.0022 mm and its volume 0.015 mm³ (c008).
const LOOSE: &[(&str, f64)] = &[("c001", 0.005), ("c002", 0.005), ("c008", 0.02)];

/// The boundary surfaces' poles are Onshape's fits: only their bodies are compared.
const BODIES_ONLY: &[&str] = &["c001", "c002"];

/// Onshape lists a loft's sides and a sheet metal part's faces in an order of its own.
const UNORDERED: &[&str] = &["c005", "c007", "c008"];

#[test]
fn batch_062_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-062");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, BODIES_ONLY, UNORDERED).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case)) && !p.starts_with("failed probe"))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        let case_gaps = KNOWN_GAPS.iter().filter(|g| g.starts_with('c')).count();
        assert_eq!(report.probes_matched, (9 - case_gaps - 1, 9), "{report}");
    });
}
