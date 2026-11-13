//! Builds fixture batch 144 (everyday surfacing and direct edits: a vase, a cable, a thickened arch, moved, deleted
//! and replaced faces, a thickened face, a drafted extrude) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Onshape measures B-spline bodies approximately: the cable (c002) is a tube, whose volume is exactly πr²L and area
/// 2πrL + 2πr² (the engine's, to 1e-12), and Onshape's are 0.17 mm³ and 0.11 mm² (4e-4, 2e-4) over; the vase
/// (c001), a fit spline revolved and shelled, is 1.0 mm³ (3e-4) off so.
const LOOSE: &[(&str, f64)] = &[("c001", 1.1), ("c002", 0.2)];

#[test]
fn batch_144_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-144");
        let report = fs_fixtures::check_batch_with(&batch, None, LOOSE).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 9), "{report}");
    });
}
