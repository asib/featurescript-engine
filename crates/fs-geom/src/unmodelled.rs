//! Builtins for things the engine doesn't model yet: sheet metal, feature patterns, tolerances, and
//! Onshape's UI (manipulators, highlighting, error display). They answer as a context without any of these would.

use crate::GeomBuiltins;
use crate::values::map;
use fs_interp::Value;

fn identity_transform() -> Value {
    let row = |i: usize| Value::array((0..3).map(|j| Value::number(if i == j { 1.0 } else { 0.0 })).collect());
    map(vec![
        ("linear", Value::array((0..3).map(row).collect())),
        ("translation", Value::array(vec![Value::number(0.0); 3])),
    ])
}

pub fn register(m: &mut GeomBuiltins) {
    m.insert("queryContainsFlattenedSheetMetal", |_, _| Ok(Value::bool(false)));
    m.insert("isInSheetMetalFeature", |_, _| Ok(Value::bool(false)));
    m.insert("isInFeaturePattern", |_, _| Ok(Value::bool(false)));
    m.insert("getRemainderPatternTransform", |_, _| Ok(identity_transform()));
    m.insert("getFullPatternTransform", |_, _| Ok(identity_transform()));
    // No tolerant parameters: the engine has no feature dialog.
    m.insert("getTolerantParameterIds", |_, _| Ok(Value::map(Default::default())));
    for name in [
        "addManipulators",
        "setDimensionedEntities",
        "setHighlightedEntities",
        "addDebugEntities",
        "transferSubfeatureErrorDisplay",
    ] {
        m.insert(name, |_, _| Ok(Value::UNDEFINED));
    }
}
