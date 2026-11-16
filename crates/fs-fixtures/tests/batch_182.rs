//! Builds fixture batch 182 (lofts with several connections) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine doesn't build as Onshape does (see docs/FINDINGS.md): a square connected twice to a hexagon
/// (c001), where each stretch's corner falls exactly between two of the hexagon's and Onshape's rounding picks the
/// earlier in one and the later in the other; two circles connected twice (c004: Onshape's side one periodic face,
/// its correspondence between the circles not even along each arc), which the engine refuses.
const KNOWN_GAPS: &[&str] = &["c001", "c004", "failed probe"];

/// A pentagon connected to a circle three times (c005) and a square at a corner and along an edge (c006): the engine's
/// arcs as with one or two connections, Onshape's spans round them laid out otherwise (four for one arc and eight for
/// the rest; uneven), so only the bodies are compared.
const BODIES_ONLY: &[&str] = &["c005", "c006"];
const LOOSE: &[(&str, f64)] = &[("c005", 0.1), ("c006", 0.2)];

/// Onshape lists the faces in its own order.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006"];

#[test]
fn batch_182_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-182");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, BODIES_ONLY, UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (4, 7), "{report}");
    });
}
