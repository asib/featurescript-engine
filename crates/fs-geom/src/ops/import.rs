//! `@opImportForeign`: an uploaded CAD file's bodies brought into the context (batches 094, 095).
//!
//! A reference to an uploaded tab (Onshape's own Import feature's `blobData`: its element id) is read from the file
//! the engine has for it. FeatureScript itself reaches no uploaded CAD data, and Onshape refuses what it can give:
//! with no foreign id the input is invalid (`INVALID_INPUT`, `foreignId`, even given an imported tab's
//! `BLOB_DATA`); a foreign id with no translation behind it (a blob's data id) fails to import
//! (`PARASOLID_IMPORT_FAILED`). The importForeign feature never gets that far: its precondition wants a
//! `ForeignId` or `CADImportData`, which a plain string or map isn't.

use crate::model::{BodyType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, field};
use crate::{occt_error, regen_error};
use fs_interp::Value;
use fs_interp::eval::{Flow, R, error};
use occt::{Shape, Transform};
use std::rc::Rc;

pub fn import_foreign(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let blob = field(definition, "blobData");
    let element = field(blob, "elementId").as_str().map(str::to_string);
    let Some(file) = element.and_then(|e| crate::uploads::file(&e)) else {
        if field(definition, "foreignId").is_undefined() {
            let message = Value::string("INVALID_INPUT").with_tag(fs_interp::types::find("ErrorStringEnum"));
            return Err(Flow::Throw(values::map(vec![
                ("message", message),
                ("faultyParameters", Value::array(vec![Value::string("foreignId")])),
            ])));
        }
        return Err(regen_error("PARASOLID_IMPORT_FAILED"));
    };
    let parts = occt::read_step(&file.to_string_lossy()).map_err(occt_error)?;
    if parts.is_empty() {
        return Err(regen_error("PARASOLID_IMPORT_FAILED"));
    }
    // The file's y up: its y is the world's z, its z the world's -y. Then its units given over.
    let scale = match field(definition, "scale") {
        v if v.is_undefined() => 1.0,
        v => v.as_number().ok_or_else(|| error("@opImportForeign: scale must be a number"))?,
    };
    let s = scale;
    let transform = if values::opt_bool(definition, "yAxisIsUp")?.unwrap_or(false) {
        Transform { m: [s, 0.0, 0.0, 0.0, 0.0, 0.0, -s, 0.0, 0.0, s, 0.0, 0.0] }
    } else {
        Transform { m: [s, 0.0, 0.0, 0.0, 0.0, s, 0.0, 0.0, 0.0, 0.0, s, 0.0] }
    };
    let identity = scale == 1.0 && !values::opt_bool(definition, "yAxisIsUp")?.unwrap_or(false);
    let info = Info::new(id);
    let mut added = Vec::new();
    for (name, shape) in parts {
        let shape: Shape =
            if identity { shape } else { occt::transform(&shape, &transform).map_err(occt_error)?.shape() };
        let mut new = NewBody::new(shape.clone(), BodyType::of(&shape), info.clone());
        if !name.is_empty() {
            Rc::make_mut(&mut new.properties).insert("NAME".into(), Value::string(name));
        }
        let b = model.add_body(new, &Origins::default(), &info, &|_| None);
        added.push(b);
    }
    if values::opt_bool(definition, "createComposite")?.unwrap_or(false) {
        let shapes: Vec<Shape> = added.iter().map(|&b| model.bodies[b].shape.clone()).collect();
        let members: Rc<[crate::model::Tid]> = added.iter().map(|&b| model.bodies[b].tid).collect();
        let mut new = NewBody::new(occt::compound(&shapes), BodyType::Composite, info.clone());
        // Closed: its parts aren't parts of their own, so a derive of them finds none (batch 096 c003, c005).
        new.composite = Some((members, true));
        if let Some(name) = field(definition, "compositeName").as_str() {
            Rc::make_mut(&mut new.properties).insert("NAME".into(), Value::string(name));
        }
        model.add_body(new, &Origins::default(), &info, &|_| None);
    }
    Ok(Value::UNDEFINED)
}
