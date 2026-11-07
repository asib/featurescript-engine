//! Runs std's own arithmetic (units, vectors, matrices, transforms, ids) and language rules through the
//! interpreter. Each check is a FeatureScript expression that must evaluate to `true`.

use fs_interp::{Engine, run_with_large_stack};

const CHECKS: &[&str] = &[
    // Language rules checked against Onshape in fixture batch 001.
    "-2^2 + 5 == 9",
    "2 * 3 % 4 == 2",
    "-7 % 3 == 2",
    "2^3^2 == 512",
    "(true ?? false && false) == true",
    "\"x\" is string == true",
    "(\"a\" ~ 1 + 2 == \"a3\") == \"afalse\"",
    // Values, tags and maps.
    "keys({ \"b\" : 1, \"a\" : 2, 3 : 0 }) == [3, \"a\", \"b\"]",
    "size({ \"a\" : undefined }) == 0",
    "BoundingType.BLIND is BoundingType",
    "\"BLIND\" as BoundingType == BoundingType.BLIND",
    "!(BoundingType.BLIND is number) && BoundingType.BLIND is string",
    "try silent(\"NOPE\" as BoundingType) == undefined",
    "(3 as number) == 3",
    "try silent(sqrt(-1)) == undefined",
    // Value semantics, boxes and closures.
    "(function() { var a = [1]; var b = a; b[0] = 2; return a[0]; })() == 1",
    "(function() { var m = { \"k\" : [1, 2] }; m.k[1] = 5; return m; })() == { \"k\" : [1, 5] }",
    "(function() { var b = new box(1); const c = b; c[] = 2; return b[]; })() == 2",
    "(function() { var x = 1; const f = function() { return x; }; x = 2; return f(); })() == 1",
    "(function() { var total = 0; for (var k, v in { \"a\" : 1, \"b\" : 2 }) { total += v; } return total; })() == 3",
    "mapArray([1, 2, 3], x => x * 2) == [2, 4, 6]",
    "[1, 2, 3]->filter(x => x > 1)->size() == 2",
    "replace(\"a~~aa~a\", \"a.\", \"X\") == \"X~X~a\"",
    "match(\"a.b[12].c\", \"(.+)(\\\\[[0-9]+\\\\]\\\\.)(.*)\").captures[2] == \"[12].\"",
    "splitByRegexp(\"fooooobazzoo\", \"oo\") == [\"f\", \"\", \"obazz\"]",
    // Units.
    "tolerantEquals(1 * meter + 2 * millimeter, 1.002 * meter)",
    "abs((2 * inch).value - 0.0508) < 1e-15",
    "isLength(5 * millimeter)",
    "!isLength(5 * degree)",
    "tolerantEquals(90 * degree, PI / 2 * radian)",
    "tolerantEquals((3 * meter) * (2 * meter), 6 * meter^2)",
    "tolerantEquals(sqrt(16 * meter^2), 4 * meter)",
    "tolerantEquals(cos(60 * degree), 0.5)",
    "(1 * meter < 2 * meter) && (2 * millimeter > 1 * millimeter)",
    "try silent(1 * meter + 1 * degree) == undefined",
    // Vectors and matrices.
    "vector(1, 2, 3) is Vector",
    "tolerantEquals(norm(vector(3, 4, 0) * meter), 5 * meter)",
    "dot(vector(1, 0, 0), vector(0, 1, 0)) == 0",
    "cross(vector(1, 0, 0), vector(0, 1, 0)) == vector(0, 0, 1)",
    "tolerantEquals(normalize(vector(0, 3, 4)), vector(0, 0.6, 0.8))",
    "tolerantEquals(rotationMatrix3d(vector(0, 0, 1), 90 * degree) * vector(1, 0, 0), vector(0, 1, 0))",
    "determinant(identityMatrix(3)) == 1",
    "squaredNorm(inverse(matrix([[2, 0], [0, 4]])) * matrix([[2, 0], [0, 4]]) - identityMatrix(2)) < 1e-24",
    // Transforms and coordinate systems.
    "tolerantEquals(transform(vector(1, 2, 3) * meter) * (vector(0, 0, 0) * meter), vector(1, 2, 3) * meter)",
    "tolerantEquals(rotationAround(Z_AXIS, 90 * degree) * (vector(1, 0, 0) * meter), vector(0, 1, 0) * meter)",
    "tolerantEquals(toWorld(coordSystem(vector(1, 0, 0) * meter, vector(0, 1, 0), vector(0, 0, 1))) * (vector(1, 0, 0) * meter), vector(1, 1, 0) * meter)",
    "XY_PLANE.normal == vector(0, 0, 1)",
    "tolerantEquals(plane(vector(0, 0, 0) * meter, vector(0, -1, 0), vector(1, 0, 0)).x, vector(1, 0, 0))",
    // Ids and contexts.
    "makeId(\"c001\") + \"sketch1\" == [\"c001\", \"sketch1\"] as Id",
    "isAtVersionOrLater(newContext(), FeatureScriptVersionNumber.V2960_DELETE_FAILED_FLAT_BY_PART)",
    "getCurrentVersion(newContext()) == FeatureScriptVersionNumberCurrent",
];

#[test]
fn std_arithmetic_and_language_rules() {
    run_with_large_stack(|| {
        let mut engine = Engine::with_vendored_std();
        let module = engine.load_geometry().expect("load geometry.fs");
        let mut failures = Vec::new();
        for check in CHECKS {
            match engine.eval(module, check) {
                Ok(v) if v.as_bool() == Some(true) => {}
                Ok(v) => failures.push(format!("{check}\n    gave {v:?}")),
                Err(e) => failures.push(format!("{check}\n    threw {e}")),
            }
        }
        assert!(failures.is_empty(), "{} of {} checks failed:\n{}", failures.len(), CHECKS.len(), failures.join("\n"));
    });
}

#[test]
fn every_std_constant_evaluates() {
    run_with_large_stack(|| {
        let mut engine = Engine::with_vendored_std();
        engine.load_geometry().expect("load geometry.fs");
        let failures = engine.check_all_constants();
        // std's defineTolerance calls its lambda with no arguments although the lambda takes one; those three
        // constants can't evaluate (see docs/FINDINGS.md).
        let expected = ["lengthTolerance", "diameterTolerance", "angleTolerance"];
        let unexpected: Vec<_> = failures.iter().filter(|f| !expected.iter().any(|name| f.contains(name))).collect();
        assert!(
            unexpected.is_empty(),
            "constants failed:\n{}",
            unexpected.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n")
        );
        assert!(engine.interp.missing_builtins.is_empty(), "missing builtins: {:?}", engine.interp.missing_builtins);
    });
}
