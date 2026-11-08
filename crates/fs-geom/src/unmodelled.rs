//! Builtins for things the engine doesn't model yet: sheet metal, tolerances, and
//! Onshape's UI (manipulators, highlighting, error display). They answer as a context without any of these would.

use crate::GeomBuiltins;
use fs_interp::Value;

pub fn register(m: &mut GeomBuiltins) {
    // Converting a value to the current FeatureScript version: the engine has only one.
    m.insert("convert", |_, args| Ok(args.first().cloned().unwrap_or(Value::UNDEFINED)));
    m.insert("queryContainsFlattenedSheetMetal", |_, _| Ok(Value::bool(false)));
    // No tolerant parameters: the engine has no feature dialog.
    m.insert("getTolerantParameterIds", |_, _| Ok(Value::map(Default::default())));
    for name in [
        "addManipulators",
        // Id disambiguation and recording queries matter only to Onshape's history.
        "skipOrderDisambiguation",
        "setExternalDisambiguation",
        "recordQuery",
        "clampContextVersion",
        "setDimensionedEntities",
        "setHighlightedEntities",
        "addDebugEntities",
        "transferSubfeatureErrorDisplay",
        // Hidden parameters only steer the feature dialog.
        "setFeatureHiddenParameters",
    ] {
        m.insert(name, |_, _| Ok(Value::UNDEFINED));
    }
}
