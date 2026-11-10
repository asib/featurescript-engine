//! Builds fixture batch 078 (slanted sheet metal bends' far ends, a loft connection and closed profiles, variable
//! fillets of a concave edge, an arc and two edges) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Onshape's variable fillets are its own fits, which stray up to 6e-4 mm from the exact sections the engine builds
/// (c007's along its arc): volumes and areas to 3e-5. The connected loft's outer bend face is 1.6e-3 mm² apart, as
/// batch 044 c003's.
const LOOSE: &[(&str, f64)] = &[("c004", 0.01), ("c006", 0.05), ("c007", 0.1), ("c008", 0.2)];

/// Faces come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c004", "c005", "c006", "c007", "c008"];

/// The L lofts' end faces lie on the engine's ruled surfaces carried past the bend to cut it, so their poles aren't
/// Onshape's; the parts match.
const BODIES_ONLY: &[&str] = &["c001", "c002", "c003"];

/// Known gaps (docs/std-coverage.md): Onshape's STEP of c001's part reads back 4% off its own area and volume (the
/// engine's match Onshape's probe); the square frustum's top ends, where both walls' edges lean out at its 86.6°
/// bends, tilt within 3.6e-5 of the volume (c005).
const KNOWN_GAPS: &[&str] = &["c001.1", "c005"];

#[test]
fn batch_078_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-078");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, BODIES_ONLY, UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|gap| p.starts_with(gap))).collect();
        assert!(unexpected.is_empty(), "{report}");
    });
}
