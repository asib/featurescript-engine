//! Builds fixture batch 055 (face blends, sphere bands extended round, curvature fillets past 0.85, loft end
//! conditions and guides) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): a sphere's band extended round it, whose new edge Onshape doesn't carry
/// each point the distance round its circle (c012–c014), and refuses at 10 mm (c015).
const KNOWN_GAPS: &[&str] = &["c012", "c013", "c014", "c015"];

/// Lofts from a square normal to the start profile only: Onshape's sides are fits whose rows rise and fall along
/// each edge by up to 0.0008 mm about the exact construction's, so its volume is 0.05 mm³ over (0.1 at magnitude
/// 2). Compared to 0.15. Curvature fillets of magnitude 0.86 to 0.95: Onshape's own areas and volumes of its
/// quintics stray, as in batch 053 (up to 0.005).
const LOOSE: &[(&str, f64)] =
    &[("c016", 0.05), ("c017", 0.05), ("c018", 0.05), ("c019", 0.05), ("c020", 0.05), ("c021", 0.15), ("c022", 0.15)];

/// Every case returns its faces' surfaces or points along its edges, to study: Onshape's B-spline approximations,
/// its faces in its own order. These are compared on their bodies only; the rest match in full, in any order (the
/// lofts between circles, c024–c026, periodic round them as Onshape's sides are, among them).
const CASES: &[&str] = &[
    "c004", "c008", "c009", "c011", "c012", "c013", "c014", "c015", "c016", "c017", "c018", "c019", "c020", "c021",
    "c022", "c023",
];

/// Compared in full, faces in any order.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c005", "c006", "c007", "c010", "c024", "c025", "c026", "c027"];

#[test]
fn batch_055_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-055");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, CASES, UNORDERED).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case)) && !p.starts_with("failed probe"))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (28 - KNOWN_GAPS.len() - 1, 28), "{report}");
    });
}
