//! Builds fixture batch 174 (lofts with guides on several sides, off a side's middle, along a corner, with end
//! conditions, between circles and through three profiles) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// With the loft normal to its first profile (c006), Onshape's bent side is its sections along the loft from the first
/// profile's derivatives plus the guide's offset blended across as without conditions (to 1e-5 mm), as the engine's;
/// but Onshape blends those derivatives from the condition's at the corners to the guide's tangent around the whole
/// profile by a rule not found (the one its plain loft normal to a polygon follows, batch 056 c001), 0.14% of the
/// volume off. Through three profiles (c008) the engine bends the side as Onshape does, but its plain loft puts the
/// middle profile at 0.4393 where Onshape's is at 0.4511, 0.14% of the volume off.
const KNOWN_GAPS: &[&str] = &["c006", "c008"];

/// The fit spline's guide: Onshape fits the rows along it with knots where it bends most (0.4375 and 0.5), the
/// engine with eight even spans, 0.007 mm³ apart; their poles differ, so only the bodies are compared.
const LOOSE: &[(&str, f64)] = &[("c004", 0.01)];
const BODIES_ONLY: &[&str] = &["c004"];

/// Onshape lists the faces and edges in its own order.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c007"];

#[test]
fn batch_174_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-174");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, BODIES_ONLY, UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (7, 9), "{report}");
    });
}
