//! Builds fixture batch 135 (uneven fillets of circles; sketches solved by their constraints) on the engine and
//! compares it with Onshape's.

use std::path::PathBuf;

/// The revolved parts' fillet rings (c003, c006) and Onshape's: OCCT's boolean between them reports nonsense; their
/// volumes, areas, boxes and topology match.
const UNCOMPARABLE: &[&str] = &["c003.1: the shapes differ", "c006.1: the shapes differ"];

/// Onshape's uneven fillets are its own fits of the sections: their parts' areas within 0.07 mm² and volumes within
/// 0.14 mm³ of the engine's, which are within 0.005 mm³ of the exact (an extruded cylinder's top, 1 and 2 mm:
/// 6256.8203 mm³ by Pappus; the engine's 6256.8154, Onshape's 6256.9501).
const LOOSE: &[(&str, f64)] = &[
    ("c001", 0.15),
    ("c002", 0.15),
    ("c003", 0.15),
    ("c004", 0.15),
    ("c005", 0.15),
    ("c006", 0.15),
    ("c007", 0.15),
    ("c008", 0.15),
    ("c009", 0.15),
    ("c010", 0.15),
];

/// The fillets' parts' faces come in an order of Onshape's own.
const UNORDERED: &[&str] = &["c001", "c002", "c003", "c004", "c005", "c006", "c007", "c008", "c009", "c010"];

#[test]
fn batch_135_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-135");
        let report = fs_fixtures::check_batch_unordered(&batch, None, LOOSE, &[], UNORDERED).unwrap();
        let unexpected: Vec<String> = report
            .problems()
            .into_iter()
            .filter(|p| !UNCOMPARABLE.iter().any(|case| p.starts_with(case)))
            .collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (21, 21), "{report}");
    });
}
