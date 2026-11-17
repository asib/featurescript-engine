//! Builds fixture batch 170 (probes: the poles of lofts from rectangles to circles) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Each side's surface is Onshape's pole for pole (cubic Hermite spans, 16 to each arc); only the point at the
/// middle of its parameters differs, Onshape's 0.06 mm up (its face's parameters normalised over a slightly larger
/// box than the surface's own).
const LOOSE: &[(&str, f64)] = &[("c001", 0.1), ("c002", 0.1), ("c003", 0.1)];

/// Onshape lists the faces in its own order.
const UNORDERED: &[&str] = &["c001", "c002", "c003"];

#[test]
fn batch_170_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-170");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (4, 4), "{report}");
    });
}
