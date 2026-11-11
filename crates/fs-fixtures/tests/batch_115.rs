//! Builds fixture batch 115 (Onshape's own curves and surfaces in an aligned section, a closed hem corner and a
//! revolved sheet extended) on the engine and compares it with Onshape's.

use std::path::PathBuf;

/// Cases the engine and Onshape don't agree on (see docs/std-coverage.md): the closed hem corner's slivers (c002)
/// are B-splines in Onshape (0.057553 mm², the engine's planes 0.057552), which its STEP writes as cylinders; the
/// revolved spline sheet extended round and past its top (c003, batch 079 c008's gap).
const KNOWN_GAPS: &[&str] = &["c002.1: faces", "c003"];

/// The closed hem corner: Onshape's hem ends are its own cubic fits, the volume 2.3e-6 off (c002, as batch 021
/// c012).
const LOOSE: &[(&str, f64)] = &[("c002", 0.02)];

/// The probes return Onshape's surfaces and curves to study: only the bodies are compared.
const BODIES_ONLY: &[&str] = &["c001", "c002", "c003"];

#[test]
fn batch_115_matches_onshape() {
    fs_interp::run_with_large_stack(|| {
        let batch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/batch-115");
        let report = fs_fixtures::check_batch_bodies(&batch, None, LOOSE, BODIES_ONLY).unwrap();
        let unexpected: Vec<String> =
            report.problems().into_iter().filter(|p| !KNOWN_GAPS.iter().any(|case| p.starts_with(case))).collect();
        assert!(unexpected.is_empty(), "{report}");
        assert_eq!(report.probes_matched, (3, 4), "{report}");
    });
}
