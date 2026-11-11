//! Builds fixture batch 062 (boundary surfaces, a non-planar fill, flats of a channel and across a bend, a
//! three-profile tangent loft) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine and Onshape don't agree on (see docs/std-coverage.md): none. A boundary surface tangent to its
/// profile, which the case leaves without `uReference`, fails in both: std finds no `createProfileConditions` taking
/// its arguments, an execution error (c003). A circle across a bend cut in the flat (c006) is wrapped onto the bend
/// as Onshape's.
const KNOWN_GAPS: &[&str] = &[];

/// Boundary surfaces between arcs: Onshape rules between its own eight-span fits of the arcs, 0.003 mm² off the
/// engine's exact arcs (c001, c002). The three-profile loft's knot across is 5e-5 from Onshape's, by a rule not
/// found: its poles are within 0.0022 mm and its volume 0.015 mm³ (c008).
/// The circle cut across a bend (c006): the hole's walls through the bend are Onshape's own fits of the wrapped
/// curve, 0.0004 mm² off the engine's wrapped faces; the part's area to 0.0016 mm², its volume to 3e-7.
const LOOSE: &[(&str, f64)] = &[("c001", 0.005), ("c002", 0.005), ("c006", 0.005), ("c008", 0.02)];

/// The boundary surfaces' poles, and the hole's walls through the bend (c006), are Onshape's fits: only their bodies
/// are compared.
const BODIES_ONLY: &[&str] = &["c001", "c002", "c006"];

/// Onshape lists a loft's sides and a sheet metal part's faces in an order of its own.
const UNORDERED: &[&str] = &["c005", "c006", "c007", "c008"];

#[test]
fn batch_062_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-062");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, BODIES_ONLY, UNORDERED).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case)))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        // Every probe line but the gaps' (the failed-cases line matches: c003 fails in both).
        let case_gaps = KNOWN_GAPS.iter().filter(|g| g.starts_with('c')).count();
        assert_eq!(report.probes_matched, (9 - case_gaps, 9), "{report}");
    });
}
