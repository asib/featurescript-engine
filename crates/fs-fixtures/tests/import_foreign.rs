//! The uploaded STEP file (fixtures/blobs/fixture-parts.step, a block "Block" and a ball "Ball") imported by the
//! engine's importForeign, against Onshape's own imports of it: the Part Studios its UI made on upload, each
//! exported. Their Import features depend on the file's tab (`blobData`, a reference to the tab, which FeatureScript
//! can't make in Onshape: batch 095); one has the import dialog's defaults (appearances on), one "Y Axis Up", one
//! a composite part (named after the file, as the feature's `compositeName` shows; the STEP has only its parts).

use fs_interp::Value;
use std::path::{Path, PathBuf};

fn blobs() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/blobs")
}

/// Imports the file with the Import feature's definition plus `options`, compares the parts with Onshape's
/// `export`, and returns the bodies' names by type (solids, composites).
fn import(options: &str, export: &Path) -> (Vec<String>, Vec<String>) {
    let mut engine = fs_geom::engine();
    fs_fixtures::register_uploads(&mut engine, &blobs()).unwrap();
    let module = engine.load_geometry().unwrap();
    let context = engine.eval(module, "newContextWithDefaults(inch)").unwrap();
    let id = engine.eval(module, "makeId(\"Import1\")").unwrap();
    // The Import feature's definition: the reference to the file's tab, the rest its defaults.
    let definition = engine
        .eval(
            module,
            &format!(
                r#"{{ "dependsOnBlob" : true, "importAppearances" : true{options},
                      "blobData" : {{ "elementId" : "aa7eee42ff6b54ecfee8ac2b", "microversionId" : "07911cc73b87f0bd2f9a105b" }} as CADImportData }}"#
            ),
        )
        .unwrap();
    engine.call(module, "importForeign", vec![context.clone(), id.clone(), definition]).unwrap();
    let status = engine.call(module, "getFeatureStatus", vec![context.clone(), id]).unwrap();
    assert_eq!(status.get("statusType").and_then(Value::as_str), Some("OK"), "{status:?}");

    use fs_geom::model::BodyType;
    let (ours, composites) = fs_geom::with_model(&context, |model| {
        let named = |kind: BodyType| -> Vec<(String, occt::Shape)> {
            model
                .bodies
                .iter()
                .filter(|b| b.body_type == kind)
                .map(|b| (b.name().unwrap_or_default().to_string(), b.shape.clone()))
                .collect()
        };
        (named(BodyType::Solid), named(BodyType::Composite))
    })
    .unwrap();
    let theirs = occt::read_step(&export.to_string_lossy()).unwrap();
    let names = |parts: &[(String, occt::Shape)]| parts.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
    assert_eq!(names(&ours), names(&theirs), "the parts and their order");
    for ((name, our), (_, their)) in ours.iter().zip(&theirs) {
        let result = fs_fixtures::compare(name, our, their);
        assert!(result.problems.is_empty(), "{name}: {:?}", result.problems);
    }
    (names(&ours), names(&composites))
}

#[test]
fn import_foreign_matches_onshapes_import() {
    fs_interp::run_with_large_stack(|| {
        let (_, composites) = import("", &blobs().join("fixture-parts-onshape.step"));
        assert!(composites.is_empty());
    });
}

#[test]
fn import_foreign_y_axis_up_matches_onshapes_import() {
    fs_interp::run_with_large_stack(|| {
        import(r#", "yAxisIsUp" : true"#, &blobs().join("fixture-parts-y-up-onshape.step"));
    });
}

#[test]
fn import_foreign_composite_matches_onshapes_import() {
    fs_interp::run_with_large_stack(|| {
        let (_, composites) = import(
            r#", "createComposite" : true, "compositeName" : "fixture-parts""#,
            &blobs().join("fixture-parts-composite-onshape.step"),
        );
        assert_eq!(composites, ["fixture-parts"]);
    });
}
