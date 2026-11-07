//! Runs fixture batch 001 on the engine: every case must build one valid solid, or fail as it did in Onshape.

use fs_geom::model::{BodyType, Model};
use fs_interp::{Engine, Value, run_with_large_stack};
use std::path::PathBuf;

/// The cases Onshape couldn't build, with the error each one gave (fixtures/batch-001/README.md).
const ONSHAPE_FAILURES: &[(&str, &str)] = &[
    ("c065", "EXTRUDE_FAILED"),
    ("c069", "EXTRUDE_FAILED"),
    ("c097", "First operand of logical or conditional operator must be boolean, value is string"),
    ("c102", "First operand of logical or conditional operator must be boolean, value is number"),
    ("c105", "EXTRUDE_FAILED"),
    ("c106", "EXTRUDE_NO_SELECTED_REGION"),
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn message(exception: &Value) -> String {
    match exception.as_str() {
        Some(s) => s.to_string(),
        None => exception.get("message").and_then(Value::as_str).unwrap_or("?").to_string(),
    }
}

fn solids(model: &Model) -> Vec<&occt::Shape> {
    model.bodies.iter().filter(|b| b.body_type == BodyType::Solid).map(|b| &b.shape).collect()
}

fn run_case(engine: &mut Engine, module: usize, name: &str) -> (Result<(), String>, usize, bool) {
    let context = engine.eval(module, "newContextWithDefaults()").unwrap();
    let id = engine.eval(module, &format!("makeId(\"{name}\")")).unwrap();
    let result = engine.call(module, name, vec![context.clone(), id]).map(|_| ()).map_err(|e| message(&e.0));
    let (count, valid) =
        fs_geom::with_model(&context, |m| (solids(m).len(), solids(m).iter().all(|s| s.is_valid()))).unwrap();
    (result, count, valid)
}

#[test]
fn batch_001_builds_what_onshape_built() {
    run_with_large_stack(|| {
        let mut engine = fs_geom::engine();
        let module = engine.load_file(root().join("fixtures/batch-001/cases.fs")).unwrap();
        let cases: Vec<String> = engine.interp.modules.list[module]
            .ast
            .items
            .iter()
            .filter_map(|item| match &item.kind {
                fs_lang::ast::ItemKind::Function { name, .. } => Some(name.name.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(cases.len(), 106);
        let mut problems = Vec::new();
        for name in &cases {
            let (result, count, valid) = run_case(&mut engine, module, name);
            match ONSHAPE_FAILURES.iter().find(|(case, _)| case == name) {
                Some((_, expected)) => {
                    if result.as_ref().err().map(String::as_str) != Some(*expected) {
                        problems.push(format!("{name}: expected {expected}, got {result:?}"));
                    }
                    if count != 0 {
                        problems.push(format!("{name}: failed but left {count} solids"));
                    }
                }
                None if result.is_err() => problems.push(format!("{name}: {}", result.unwrap_err())),
                None if count != 1 || !valid => problems.push(format!("{name}: {count} solids, valid: {valid}")),
                None => {}
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    });
}

#[test]
fn a_failed_feature_leaves_no_geometry() {
    run_with_large_stack(|| {
        let mut engine = fs_geom::engine();
        let source = r#"FeatureScript 3112;
import(path : "onshape/std/geometry.fs", version : "3112.0");

export function build(context is Context, id is Id, depth is ValueWithUnits)
{
    const sketch = newSketchOnPlane(context, id + "sketch", { "sketchPlane" : XY_PLANE });
    skRectangle(sketch, "rect", { "firstCorner" : vector(0, 0) * millimeter, "secondCorner" : vector(10, 20) * millimeter });
    skSolve(sketch);
    try silent
    {
        extrude(context, id + "extrude", { "entities" : qSketchRegion(id + "sketch"), "endBound" : BoundingType.BLIND, "depth" : depth });
    }
    return size(evaluateQuery(context, qCreatedBy(id + "extrude", EntityType.BODY)));
}
"#;
        let module = engine.load_source("rollback.fs", source).unwrap();
        let context = engine.eval(module, "newContextWithDefaults()").unwrap();
        for (name, depth, expected) in [("zero", "0 * millimeter", 0.0), ("five", "5 * millimeter", 1.0)] {
            let id = engine.eval(module, &format!("makeId(\"{name}\")")).unwrap();
            let depth = engine.eval(module, depth).unwrap();
            let built = engine.call(module, "build", vec![context.clone(), id, depth]).unwrap();
            assert_eq!(built.as_number(), Some(expected), "{name}");
        }
        let (count, volume) =
            fs_geom::with_model(&context, |m| (solids(m).len(), solids(m)[0].volume().unwrap())).unwrap();
        assert_eq!(count, 1);
        assert!((volume - 10e-3 * 20e-3 * 5e-3).abs() < 1e-15, "{volume}");
    });
}
