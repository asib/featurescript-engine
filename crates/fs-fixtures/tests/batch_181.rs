//! Builds fixture batch 181 (lofts between polygons of different corner counts, and polygons connected to circles) on
//! the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine doesn't build as Onshape does yet: none.
const KNOWN_GAPS: &[&str] = &[];

/// A square connected to a circle twice (c007, c008: each arc between the connections shared evenly among the edges
/// between, eight spans to each) and a rectangle once (c009): their sides Onshape's pole for pole, Onshape's circular
/// end measured on other approximations of the arcs, as in batch 180 (c008's volume 0.01 mm³ off).
const LOOSE: &[(&str, f64)] = &[("c007", 0.002), ("c008", 0.01), ("c009", 0.1)];

/// Onshape lists the faces in its own order.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008", "c009"];

#[test]
fn batch_181_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-181");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (10, 10), "{report}");
    });
}
