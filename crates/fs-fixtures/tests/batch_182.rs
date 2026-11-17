//! Builds fixture batch 182 (lofts with several connections) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine doesn't build as Onshape does (see docs/FINDINGS.md): a square connected twice to a hexagon
/// (c001), where each stretch's corner falls exactly between two of the hexagon's and Onshape's rounding picks the
/// earlier in one and the later in the other.
const KNOWN_GAPS: &[&str] = &["c001"];

/// Onshape's circular ends measured on other approximations of the arcs than its sides' edges (as in batch 180):
/// c005's volume 0.010 mm³ off, c006's 0.009 and its end's area 0.0024 mm².
const LOOSE: &[(&str, f64)] = &[("c005", 0.005), ("c006", 0.005)];

/// Onshape lists the faces in its own order.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006"];

#[test]
fn batch_182_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-182");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (6, 7), "{report}");
    });
}
