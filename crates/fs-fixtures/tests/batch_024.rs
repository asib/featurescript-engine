//! Builds fixture batch 024 (tapped, clearance and V3 holes, revolve start bounds, an aligned gusset, ruled surface
//! vertex overrides, a planar fill and a wrap onto a cone) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases that differ (see docs/std-coverage.md): where Onshape puts the origin of a planar sheet that isn't
/// symmetric. The engine takes the middle of the face's box; Onshape's is 0.3% of the box's size off that (c008 and
/// c009, the ruled sheets with vertex overrides; c010, a triangle). The sheets themselves match.
const KNOWN_GAPS: &[&str] = &["c008 probe: c008.2 plane", "c009 probe: c009.2 plane", "c010 probe: c010.1 plane"];

#[test]
fn batch_024_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-024");
        let report = fs_fixtures::check_batch(&batch, None).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|gap| p.starts_with(gap))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (9, 12), "{report}");
    });
}
