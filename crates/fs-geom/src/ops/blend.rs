//! `@opFillet` and `@opChamfer`.

use super::{by_body, replace_bodies};
use crate::model::{EntityRef, EntityType, FeatureId, Model, Tid};
use crate::values::{self, field};
use crate::{query, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Shape, ShapeKind};

/// The edges to blend: edges selected, and every edge of faces selected, grouped by body.
fn blend_edges(model: &Model, definition: &Value) -> R<Vec<(usize, Vec<Shape>)>> {
    let entities = field(definition, "entities");
    let mut groups = by_body(model, entities, EntityType::Edge)?;
    for (b, faces) in by_body(model, entities, EntityType::Face)? {
        let edges: Vec<Shape> = faces.iter().flat_map(|f| f.visible_sub_shapes(ShapeKind::Edge)).collect();
        match groups.iter_mut().find(|(g, _)| *g == b) {
            Some((_, list)) => list.extend(edges),
            None => groups.push((b, edges)),
        }
    }
    for (_, edges) in &mut groups {
        let mut unique: Vec<Shape> = Vec::new();
        for e in edges.drain(..) {
            if !unique.iter().any(|u| u.is_same(&e)) {
                unique.push(e);
            }
        }
        *edges = unique;
    }
    Ok(groups)
}

pub fn fillet(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    match field(definition, "crossSection").as_str() {
        None | Some("CIRCULAR") => {}
        Some(other) => return Err(error(format!("@opFillet: {other} cross sections aren't supported yet"))),
    }
    for key in ["isVariable", "isAsymmetric", "createDetachedSurface"] {
        if values::opt_bool(definition, key)?.unwrap_or(false) {
            return Err(error(format!("@opFillet: {key} isn't supported yet")));
        }
    }
    if field(definition, "partialFilletBounds").as_array().is_some_and(|b| !b.is_empty()) {
        return Err(error("@opFillet: partial fillets aren't supported yet"));
    }
    let radius = values::length(field(definition, "radius"), "opFillet radius")?;
    let groups = blend_edges(model, definition)?;
    if groups.is_empty() {
        return Err(regen_error("FILLET_SELECT_EDGES"));
    }
    let mut results = Vec::new();
    for (b, edges) in groups {
        let op = occt::fillet(&model.bodies[b].shape, &edges, radius).map_err(|_| regen_error("FILLET_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

/// The face next to an edge that the edge runs forward along: walking along the edge with that face's normal up,
/// the face is on the left. Uneven chamfers measure their first distance on it.
fn left_face(body: &Shape, edge: &Shape) -> Option<Shape> {
    body.ancestors(edge, ShapeKind::Face).into_iter().find(|face| {
        face.sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(edge) && e.orientation() == edge.orientation())
    })
}

pub fn chamfer(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let width = |key: &str| values::length(field(definition, key), key);
    let opposite = values::opt_bool(definition, "oppositeDirection")?.unwrap_or(false);
    let kind = match field(definition, "chamferType").as_str() {
        Some("EQUAL_OFFSETS") | None => occt::Chamfer::Equal(width("width")?),
        Some("TWO_OFFSETS") => {
            let (a, b) = (width("width1")?, width("width2")?);
            if opposite { occt::Chamfer::TwoDistances(b, a) } else { occt::Chamfer::TwoDistances(a, b) }
        }
        Some("OFFSET_ANGLE") => {
            occt::Chamfer::DistanceAngle(width("width")?, values::angle(field(definition, "angle"), "angle")?)
        }
        Some(other) => return Err(error(format!("@opChamfer: {other} isn't supported yet"))),
    };
    let groups = blend_edges(model, definition)?;
    if groups.is_empty() {
        return Err(regen_error("CHAMFER_SELECT_EDGES"));
    }
    let mut results = Vec::new();
    for (b, edges) in groups {
        let body = model.bodies[b].shape.clone();
        let mut faces = Vec::new();
        for edge in &edges {
            let mut face = left_face(&body, edge).ok_or_else(|| regen_error("CHAMFER_FAILED"))?;
            if opposite && matches!(kind, occt::Chamfer::DistanceAngle(..)) {
                face = body
                    .ancestors(edge, ShapeKind::Face)
                    .into_iter()
                    .find(|f| !f.is_same(&face))
                    .ok_or_else(|| regen_error("CHAMFER_FAILED"))?;
            }
            faces.push(face);
        }
        let op = occt::chamfer(&body, &edges, &faces, kind).map_err(|_| regen_error("CHAMFER_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

/// `@opModifyFillet`: removes fillets (the faces are deleted and their neighbours extended to meet), or changes
/// their radius (removed, then the edge where their tangent neighbours now meet is filleted again).
pub fn modify_fillet(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let change = match field(definition, "modifyFilletType").as_str() {
        Some("REMOVE_FILLET") => None,
        Some("CHANGE_RADIUS") => Some(values::length(field(definition, "radius"), "opModifyFillet radius")?),
        other => return Err(error(format!("@opModifyFillet: modifyFilletType {other:?} isn't supported"))),
    };
    let fillets = query::evaluate(model, field(definition, "faces"))?;
    let fillets: Vec<EntityRef> = fillets.into_iter().filter(|r| r.kind == EntityType::Face).collect();
    if fillets.is_empty() {
        return Err(regen_error("MODIFY_FILLET_SELECT_FACES"));
    }
    // For each fillet, the transient ids of the two faces it is tangent to.
    let mut rails: Vec<(Tid, Tid)> = Vec::new();
    for &fillet in &fillets {
        let face = model.shape(fillet);
        let body = model.body(fillet);
        let mut found = Vec::new();
        for edge in face.visible_sub_shapes(ShapeKind::Edge) {
            for (index, other) in body.faces.iter().enumerate() {
                let r = EntityRef { body: fillet.body, kind: EntityType::Face, index };
                if fillets.contains(&r) || other.shape.is_same(face) {
                    continue;
                }
                let shares = other.shape.visible_sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(&edge));
                if shares && edge.faces_tangent_along(face, &other.shape, 1e-6) && !found.contains(&other.tid) {
                    found.push(other.tid);
                }
            }
        }
        if let [a, b] = found[..] {
            rails.push((a, b));
        } else if change.is_some() {
            return Err(regen_error("MODIFY_FILLET_FAILED"));
        }
    }
    let mut results = Vec::new();
    for (b, faces) in by_body(model, field(definition, "faces"), EntityType::Face)? {
        let op = occt::delete_faces(&model.bodies[b].shape, &faces).map_err(|_| regen_error("MODIFY_FILLET_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    let Some(radius) = change else { return Ok(Value::UNDEFINED) };
    // The rails' images now meet at a sharp edge.
    let image = |model: &Model, tid: Tid| -> Option<EntityRef> {
        model.everything(Some(EntityType::Face)).into_iter().find(|r| {
            let entity = model.entity(*r).expect("a face");
            entity.tid == tid || entity.ancestry.identity.contains(&tid)
        })
    };
    let mut edges: Vec<(usize, Shape)> = Vec::new();
    for (a, b) in rails {
        let (Some(a), Some(b)) = (image(model, a), image(model, b)) else {
            return Err(regen_error("MODIFY_FILLET_FAILED"));
        };
        let theirs = model.shape(b).visible_sub_shapes(ShapeKind::Edge);
        for edge in model.shape(a).visible_sub_shapes(ShapeKind::Edge) {
            if theirs.iter().any(|e| e.is_same(&edge)) {
                edges.push((a.body, edge));
            }
        }
    }
    let mut results: Vec<(usize, occt::Operation)> = Vec::new();
    let mut by_body: Vec<(usize, Vec<Shape>)> = Vec::new();
    for (b, edge) in edges {
        match by_body.iter_mut().find(|(g, _)| *g == b) {
            Some((_, list)) => list.push(edge),
            None => by_body.push((b, vec![edge])),
        }
    }
    for (b, edges) in by_body {
        let op =
            occt::fillet(&model.bodies[b].shape, &edges, radius).map_err(|_| regen_error("MODIFY_FILLET_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}
