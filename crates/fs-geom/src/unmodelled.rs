//! Builtins for things the engine doesn't model yet: attributes, sheet metal, feature patterns, tolerances, and
//! Onshape's UI (manipulators, highlighting, error display). They answer as a context without any of these would.

use crate::GeomBuiltins;
use crate::values::map;
use fs_interp::Value;
use fs_interp::eval::error;

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
    // Attributes: nothing can set one yet, so every entity has none.
    m.insert("getAttributes", |_, _| Ok(Value::array(Vec::new())));
    m.insert("getAttribute", |_, _| Ok(Value::UNDEFINED));
    m.insert("getAllAttributes", |_, _| Ok(Value::map(Default::default())));
    m.insert("removeAttributes", |_, _| Ok(Value::UNDEFINED));
    m.insert("setAttribute", |_, _| Err(error("@setAttribute: attributes aren't supported yet")));
}
