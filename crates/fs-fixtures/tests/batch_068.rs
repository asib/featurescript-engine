//! Builds fixture batch 068 (a rolled wall between flat walls unfolded and refolded, a shelled box recognized) on
//! the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine and Onshape don't agree on (see docs/std-coverage.md): an open box shelled from a block, which
/// Onshape won't recognize as sheet metal (`SHEET_METAL_CANNOT_RECOGNIZE_PARTS`: three sharp walls meet at each
/// corner) and the engine does (c003). c001 and c002 fail in both alike: the walls either side of the rolled wall
/// are parts of their own (rips at the tangent joints), so the hold face isn't on the rolled wall's part.
const KNOWN_GAPS: &[&str] = &["c003"];

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
        assert_eq!(report.probes_matched, (2, 4), "{report}");
    });
}
