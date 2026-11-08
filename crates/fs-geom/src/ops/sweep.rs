//! `@opSweep` and `@opLoft`.

use super::ShapeSet;
use crate::model::{BodyType, Cap, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, field};
use crate::{occt_error, query, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Shape, ShapeKind};
use std::rc::Rc;

/// Adds an operation's solids (or, for sheets, shells) with start and end cap infos.
fn add_with_caps(model: &mut Model, id: &FeatureId, op: &occt::Operation, solid: bool) -> R<()> {
    let (mut start_cap, mut end_cap) = (ShapeSet::default(), ShapeSet::default());
    for face in op.first() {
        start_cap.insert_all(&face);
    }
    for face in op.last() {
        end_cap.insert_all(&face);
    }
    let created = Info::new(id.clone());
    let cap = |c| Rc::new(Info { created_by: id.clone(), sketch_entity: None, cap: Some(c), dependency: None });
    let (start_info, end_info) = (cap(Cap::Start), cap(Cap::End));
    let special = |s: &Shape| {
        if end_cap.contains(s) {
            Some(end_info.clone())
        } else if start_cap.contains(s) {
            Some(start_info.clone())
        } else {
            None
        }
    };
    let (kind, body_type) =
        if solid { (ShapeKind::Solid, BodyType::Solid) } else { (ShapeKind::Shell, BodyType::Sheet) };
    let mut pieces = op.shape().sub_shapes(kind);
    if pieces.is_empty() && !solid {
        pieces = op.shape().sub_shapes(ShapeKind::Face);
    }
    if pieces.is_empty() {
        return Err(regen_error("SWEEP_FAILED"));
    }
    for piece in pieces {
        model.add_body(NewBody::new(piece, body_type, created.clone()), &Origins::default(), &created, &special);
    }
    Ok(())
}

pub fn sweep(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    if values::opt_bool(definition, "keepProfileOrientation")?.unwrap_or(false) {
        return Err(error("@opSweep: keepProfileOrientation isn't supported yet"));
    }
    match field(definition, "profileControl").as_str() {
        None | Some("NONE") => {}
        Some(other) => return Err(error(format!("@opSweep: profileControl {other} isn't supported yet"))),
    }
    let found = query::evaluate(model, field(definition, "profiles"))?;
    let faces: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Face).map(|r| model.shape(*r).clone()).collect();
    let edges: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Edge).map(|r| model.shape(*r).clone()).collect();
    let path_edges: Vec<Shape> = query::evaluate(model, field(definition, "path"))?
        .iter()
        .filter(|r| r.kind == EntityType::Edge)
        .map(|r| model.shape(*r).clone())
        .collect();
    let paths = occt::connect_edges(&path_edges).map_err(occt_error)?;
    let [path] = paths.as_slice() else { return Err(regen_error("SWEEP_PATH_NO_CONSTRAINT")) };
    let solid = !faces.is_empty();
    let profiles: Vec<Shape> = if solid {
        if faces.len() == 1 { faces } else { vec![occt::merge_faces(&faces).map_err(occt_error)?.shape()] }
    } else {
        occt::connect_edges(&edges).map_err(occt_error)?
    };
    if profiles.is_empty() {
        return Err(regen_error("SWEEP_SELECT_PROFILE"));
    }
    for profile in &profiles {
        let op = occt::sweep(profile, path).map_err(|_| regen_error("SWEEP_FAILED"))?;
        add_with_caps(model, &id, &op, solid)?;
    }
    Ok(Value::UNDEFINED)
}

pub fn loft(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    for key in ["guideSubqueries", "connections", "derivativeInfo"] {
        if field(definition, key).as_array().is_some_and(|a| !a.is_empty()) {
            return Err(error(format!("@opLoft: {key} isn't supported yet")));
        }
    }
    let solid = field(definition, "bodyType").as_str() != Some("SURFACE");
    let subqueries = field(definition, "profileSubqueries")
        .as_array()
        .ok_or_else(|| error("@opLoft: profileSubqueries must be an array"))?;
    let mut wires = Vec::new();
    for q in subqueries.iter() {
        let found = query::evaluate(model, q)?;
        let faces: Vec<&_> = found.iter().filter(|r| r.kind == EntityType::Face).collect();
        if let Some(face) = faces.first() {
            if faces.len() > 1 {
                return Err(error("@opLoft: profiles of several faces aren't supported yet"));
            }
            wires.push(occt::outer_wire(model.shape(**face)).map_err(occt_error)?);
            continue;
        }
        let edges: Vec<Shape> =
            found.iter().filter(|r| r.kind == EntityType::Edge).map(|r| model.shape(*r).clone()).collect();
        if edges.is_empty() {
            // A point profile (a vertex): not supported yet.
            return Err(error("@opLoft: point profiles aren't supported yet"));
        }
        let mut connected = occt::connect_edges(&edges).map_err(occt_error)?;
        if connected.len() != 1 {
            return Err(regen_error("LOFT_PROFILE_FAILED"));
        }
        wires.push(connected.remove(0));
    }
    if wires.len() < 2 {
        return Err(regen_error("LOFT_SELECT_PROFILES"));
    }
    let op = occt::loft(&wires, solid).map_err(|_| regen_error("LOFT_FAILED"))?;
    add_with_caps(model, &id, &op, solid)?;
    Ok(Value::UNDEFINED)
}
