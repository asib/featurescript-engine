//! Builds fixture batch 117 (boundary surfaces between arcs with end conditions at both profiles, the second, and
//! along directions) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Normal to both profiles' planes (c001) and to the second's (c003): Onshape's surfaces are built on its own fits
/// of the arcs, 0.0002 and 0.0015 mm² off the engine's exact ones. At magnitudes 0.5 and 1.5 (c006) the engine fits
/// the leaving derivatives as Onshape fits other magnitudes (batches 162, 163) on its own magnitude-1 derivative,
/// whose interior is 5e-3 off Onshape's: 0.0056 mm² apart (magnitude 2 at the first alone, c002, within 1e-3). Along
/// a direction (c004), its free end reached as with a condition normal to a plane (batch 159 c005's blend), 0.058 mm²
/// off Onshape's adaptive fit.
const LOOSE: &[(&str, f64)] = &[("c001", 0.005), ("c003", 0.005), ("c004", 0.1), ("c006", 0.01)];

/// The probes return Onshape's poles, its own fits: only the bodies are compared, but for c005, which fails in both.
const BODIES_ONLY: &[&str] = &["c001", "c002", "c003", "c004", "c006"];

/// Compared in full, faces in any order.
const UNORDERED: &[&str] = &["c005"];

#[test]
fn batch_117_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-117");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, BODIES_ONLY, UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        // c005 fails in both (BSURF_INCONSISTENT_CONDITIONS_ERROR).
        assert_eq!(report.probes_matched, (7, 7), "{report}");
    });
}
