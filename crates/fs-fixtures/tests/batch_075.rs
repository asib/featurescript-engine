//! Builds fixture batch 075 (lofts with valid connections, guides and paths) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Cases the engine doesn't build as Onshape does: a guide bulging out (c005: Onshape curves the face it lies on,
/// the others stay planar; the engine doesn't support guides that change the loft); circles lofted along a straight
/// path (c006: Onshape's side is its own fit through sections, 5493.27 mm³ to the cone's 5497.79). Along a quarter
/// circle (c007) both fail (`LOFT_FAILED`, as for batch 074 c006).
const KNOWN_GAPS: &[&str] = &["c005", "c006", "failed probe"];
/// The lofts' faces come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004"];

#[test]
fn batch_075_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-075");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        let problems: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|g| p.starts_with(g))).collect();
        assert!(problems.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (5, 8), "{report}");
    });
}
