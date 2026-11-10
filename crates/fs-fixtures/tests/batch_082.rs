//! Builds fixture batch 082 (conic, curvature and asymmetric fillets of a circular edge; a face blend with two
//! solutions; circles lofted with a connection) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Onshape's fillets of the cylinder's top edge are its own fits: their circles bound its top 0.03 mm² larger than
/// the exact one, volumes to 8e-6. Its twisted loft between the circles is its own too, 2.7e-5 under the exact
/// ruled solid's volume.
const LOOSE: &[(&str, f64)] = &[("c001", 0.05), ("c002", 0.05), ("c003", 0.06), ("c006", 0.15)];

/// Faces and curved edges come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c006"];

#[test]
fn batch_082_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-082");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], UNORDERED).unwrap();
        assert!(report.problems().is_empty(), "{report}");
        assert_eq!(report.probes_matched, (7, 7), "{report}");
    });
}
