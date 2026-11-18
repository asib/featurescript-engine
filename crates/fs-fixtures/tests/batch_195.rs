//! Builds fixture batch 195 (rational B-spline curves trimmed and extended; offset curves across other curved faces)
//! on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Offsets whose curves are of other kinds than Onshape's, compared on their bodies: the rulings offset round an
/// extruded elliptical arc and an extruded fit spline (c008, c010), lines in the engine, which the probe leaves out,
/// curves of another kind on the same lines (within 1e-5 mm) in Onshape; offsets round a revolved fit spline (c011:
/// its meridian; c012: its bottom circle, the exact parallel), B-splines in the engine, where Onshape's curves are
/// its own fits of another kind, their points within 2.1e-4 and 2.7e-4 mm of the engine's, their lengths 3.9e-4 and
/// 8.7e-5 mm off.
const BODIES_ONLY: &[&str] = &["c008", "c010", "c011", "c012"];
const LOOSE: &[(&str, f64)] = &[("c011", 5e-4), ("c012", 5e-4)];

#[test]
fn batch_195_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-195");
        let report = fs_fixtures::check_batch_bodies(&batch, None, LOOSE, BODIES_ONLY).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (13, 13), "{report}");
    });
}
