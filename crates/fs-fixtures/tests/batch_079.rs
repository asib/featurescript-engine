//! Builds fixture batch 079 (face blends with an end cap, a cliff edge, a help point and a hold line; an elliptical
//! arc extended; a revolved sheet extended) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Faces come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c003", "c004"];

/// Known gaps (docs/std-coverage.md): Onshape fails a blend between a floor and a cylinder lying through it
/// (c005, FACE_BLEND_FAILED) where the engine doesn't blend planes with cylinders that way at all; a revolved spline
/// sheet extended round and past its top (c008) is within 5.9e-3 of Onshape's area, which goes round further at
/// its widest and rises higher, yet covers less.
const KNOWN_GAPS: &[&str] = &["c005", "c008"];

#[test]
fn batch_079_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-079");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|gap| p.starts_with(gap))).collect();
        assert!(unexpected.is_empty(), "{report}");
    });
}
