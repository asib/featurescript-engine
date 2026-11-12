//! Builds fixture batch 132 (conic, curvature and uneven fillets of curved edges) on the engine and compares it with
//! Onshape's.

use std::path::PathBuf;

/// Cases the engine can't build as Onshape does yet (see docs/std-coverage.md): a cone frustum's top edge filleted
/// unevenly (c009), which Onshape measures `radius` from the top where the engine measures it from the cone.
const KNOWN_GAPS: &[&str] = &["c009"];

/// The balls' fillet ring (c010) and Onshape's: OCCT's boolean between them reports nonsense, and its distance from
/// a point on either ring to the other part misses that ring for the spheres; their volumes, areas, boxes and
/// topology match.
const UNCOMPARABLE: &[&str] = &["c010.1: the shapes differ"];

/// Onshape's fillet faces are its own fits of the sections, 4e-5 to 7e-5 of their areas under the exact surfaces
/// the engine's lie on (as in batch 082): the parts' areas within 0.06 mm², volumes within 0.04 mm³.
const LOOSE: &[(&str, f64)] = &[
    ("c001", 0.06),
    ("c002", 0.06),
    ("c003", 0.06),
    ("c004", 0.06),
    ("c005", 0.06),
    ("c006", 0.06),
    ("c007", 0.06),
    ("c008", 0.06),
    ("c009", 0.06),
    ("c010", 0.06),
];

/// Each case returns its fillet's faces' areas and its edges' points at eight places round, to study; where a
/// closed edge starts differs (Onshape's are its own), so only the bodies are compared.
const CASES: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008", "c009", "c010"];

#[test]
fn batch_132_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-132");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, CASES, &[]).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !KNOWN_GAPS.iter().chain(UNCOMPARABLE).any(|case| p.starts_with(case)))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (11 - KNOWN_GAPS.len(), 11), "{report}");
    });
}
