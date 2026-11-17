//! Builds fixture batch 185 (sketch text in the other fonts) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Fonts the engine doesn't have as Onshape does (fs-geom/fonts/README.md): Arimo Bold (c004), Noto Sans and Noto
/// Serif (c011–c018), whose versions in Debian's packages aren't Onshape's, and Allerta Stencil, Droid Sans Mono and
/// Roboto Slab (c019–c021). The engine refuses them all, which the failed cases' line shows.
const KNOWN_GAPS: &[&str] = &[
    "c004", "c011", "c012", "c013", "c014", "c015", "c016", "c017", "c018", "c019", "c020", "c021", "failed probe",
];

/// Onshape lists the regions in its own order.
const UNORDERED: &[&str] = &[
    "c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008", "c009", "c010", "c011", "c012", "c013", "c014",
    "c015", "c016", "c017", "c018", "c019", "c020", "c021", "c022",
];

#[test]
fn batch_185_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-185");
        let report = fs_fixtures::check_batch_unordered(&batch, None, &[], &[], UNORDERED).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (10, 23), "{report}");
    });
}
