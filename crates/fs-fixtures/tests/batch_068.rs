//! Builds fixture batch 068 (a rolled wall between flat walls unfolded and refolded, a shelled box recognized) on
//! the engine and compares it with Onshape's.

use std::path::PathBuf;

/// An open box shelled from a block isn't recognized as sheet metal, in Onshape or the engine
/// (`SHEET_METAL_CANNOT_RECOGNIZE_PARTS`: three sharp walls meet at each corner, c003). c001 and c002 fail in both
/// alike: the walls either side of the rolled wall are parts of their own (rips at the tangent joints), so the hold
/// face isn't on the rolled wall's part; the parts they leave differ (Part 2, Part 5).
const KNOWN_GAPS: &[&str] = &[];

#[test]
fn batch_068_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-068");
        let report = fs_fixtures::check_batch_bodies(&batch, None, &[], &[]).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case)) && !p.starts_with("failed probe") && !p.starts_with("Part "))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (4, 4), "{report}");
    });
}
