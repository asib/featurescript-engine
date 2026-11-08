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
    for key in ["isAsymmetric", "createDetachedSurface"] {
        if values::opt_bool(definition, key)?.unwrap_or(false) {
            return Err(error(format!("@opFillet: {key} isn't supported yet")));
        }
    }
    if values::opt_bool(definition, "isVariable")?.unwrap_or(false) {
        return variable_fillet(model, id, definition);
    }
    if field(definition, "partialFilletBounds").as_array().is_some_and(|b| !b.is_empty()) {
        return partial_fillet(model, id, definition);
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
    let refillet = change.is_some() && values::opt_bool(definition, "reFillet")?.unwrap_or(false);
    let fillets = query::evaluate(model, field(definition, "faces"))?;
    let fillets: Vec<EntityRef> = fillets.into_iter().filter(|r| r.kind == EntityType::Face).collect();
    if fillets.is_empty() {
        return Err(regen_error("MODIFY_FILLET_SELECT_FACES"));
    }
    // Refilleting, the fillets next to these (curved faces tangent to them, and on through tangent curved faces)
    // are taken away too and made again afterwards with their own radii (batch 028).
    let neighbours: Vec<EntityRef> = if refillet { tangent_curved_neighbours(model, &fillets) } else { Vec::new() };
    let all: Vec<EntityRef> = fillets.iter().chain(neighbours.iter()).copied().collect();
    let rails_of = |fillet: EntityRef| -> Vec<Tid> {
        let face = model.shape(fillet);
        let body = model.body(fillet);
        let mut found = Vec::new();
        for edge in face.visible_sub_shapes(ShapeKind::Edge) {
            for (index, other) in body.faces.iter().enumerate() {
                let r = EntityRef { body: fillet.body, kind: EntityType::Face, index };
                if all.contains(&r) || other.shape.is_same(face) {
                    continue;
                }
                let shares = other.shape.visible_sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(&edge));
                if shares && edge.faces_tangent_along(face, &other.shape, 1e-6) && !found.contains(&other.tid) {
                    found.push(other.tid);
                }
            }
        }
        found
    };
    // For each fillet, the transient ids of the two faces it is tangent to.
    let mut rails: Vec<(Tid, Tid)> = Vec::new();
    for &fillet in &fillets {
        if let [a, b] = rails_of(fillet)[..] {
            rails.push((a, b));
        } else if change.is_some() {
            return Err(regen_error("MODIFY_FILLET_FAILED"));
        }
    }
    // The neighbours made again: the cylinders among them, by their rails and radius (their corners come back as
    // the fillets meet).
    let mut again: Vec<(Tid, Tid, f64)> = Vec::new();
    for &n in &neighbours {
        let Ok(surface) = model.shape(n).surface_definition() else { continue };
        if surface.kind != 1 {
            continue;
        }
        if let [a, b] = rails_of(n)[..]
            && !again.iter().any(|(x, y, _)| (*x, *y) == (a, b) || (*x, *y) == (b, a))
        {
            again.push((a, b, surface.radius1));
        }
    }
    let mut results = Vec::new();
    let mut groups: Vec<(usize, Vec<Shape>)> = Vec::new();
    for r in &all {
        let shape = model.shape(*r).clone();
        match groups.iter_mut().find(|(b, _)| *b == r.body) {
            Some((_, list)) => list.push(shape),
            None => groups.push((r.body, vec![shape])),
        }
    }
    for (b, faces) in groups {
        let op = occt::delete_faces(&model.bodies[b].shape, &faces).map_err(|_| regen_error("MODIFY_FILLET_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    let Some(radius) = change else { return Ok(Value::UNDEFINED) };
    refillet_rails(model, &id, rails.iter().map(|&(a, b)| (a, b, radius)).collect())?;
    let mut radii: Vec<f64> = Vec::new();
    for &(_, _, r) in &again {
        if !radii.iter().any(|x| (x - r).abs() < 1e-12) {
            radii.push(r);
        }
    }
    for r in radii {
        refillet_rails(model, &id, again.iter().filter(|g| (g.2 - r).abs() < 1e-12).copied().collect())?;
    }
    Ok(Value::UNDEFINED)
}

/// The curved faces tangent to these fillets, and on through curved faces tangent to those.
fn tangent_curved_neighbours(model: &Model, fillets: &[EntityRef]) -> Vec<EntityRef> {
    let mut found: Vec<EntityRef> = Vec::new();
    let mut queue: Vec<EntityRef> = fillets.to_vec();
    while let Some(r) = queue.pop() {
        let face = model.shape(r).clone();
        let body = model.body(r).clone();
        for edge in face.visible_sub_shapes(ShapeKind::Edge) {
            for (index, other) in body.faces.iter().enumerate() {
                let o = EntityRef { body: r.body, kind: EntityType::Face, index };
                if fillets.contains(&o) || found.contains(&o) || other.shape.is_same(&face) {
                    continue;
                }
                if other.shape.face_plane().is_ok() {
                    continue;
                }
                let shares = other.shape.visible_sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(&edge));
                if shares && edge.faces_tangent_along(&face, &other.shape, 1e-6) {
                    found.push(o);
                    queue.push(o);
                }
            }
        }
    }
    found
}

/// Fillets the edges where each pair of faces (by transient id, through what became of them) now meets.
fn refillet_rails(model: &mut Model, id: &FeatureId, rails: Vec<(Tid, Tid, f64)>) -> R<()> {
    let image = |model: &Model, tid: Tid| -> Option<EntityRef> {
        model.everything(Some(EntityType::Face)).into_iter().find(|r| {
            let entity = model.entity(*r).expect("a face");
            entity.tid == tid || entity.ancestry.identity.contains(&tid)
        })
    };
    let mut by_body: Vec<(usize, Vec<Shape>, f64)> = Vec::new();
    for (a, b, radius) in rails {
        let (Some(a), Some(b)) = (image(model, a), image(model, b)) else {
            return Err(regen_error("MODIFY_FILLET_FAILED"));
        };
        let theirs = model.shape(b).visible_sub_shapes(ShapeKind::Edge);
        for edge in model.shape(a).visible_sub_shapes(ShapeKind::Edge) {
            if theirs.iter().any(|e| e.is_same(&edge)) {
                match by_body.iter_mut().find(|(g, _, _)| *g == a.body) {
                    Some((_, list, _)) => list.push(edge),
                    None => by_body.push((a.body, vec![edge], radius)),
                }
            }
        }
    }
    let mut results: Vec<(usize, occt::Operation)> = Vec::new();
    for (b, edges, radius) in by_body {
        let op =
            occt::fillet(&model.bodies[b].shape, &edges, radius).map_err(|_| regen_error("MODIFY_FILLET_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, id, results);
    Ok(())
}

/// `@opFullRoundFillet`: rounds the centre faces between two parallel planar side faces into a half cylinder
/// tangent to both.
pub fn full_round(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let one = |key: &str| -> R<Option<EntityRef>> {
        Ok(query::evaluate(model, field(definition, key))?.into_iter().find(|r| r.kind == EntityType::Face))
    };
    let (Some(side1), Some(side2)) = (one("side1Face")?, one("side2Face")?) else {
        return Err(regen_error("FULL_ROUND_FILLET_SELECT_FACES"));
    };
    let centre: Vec<Shape> = query::evaluate(model, field(definition, "centerFaces"))?
        .into_iter()
        .filter(|r| r.kind == EntityType::Face && r.body == side1.body)
        .map(|r| model.shape(r).clone())
        .collect();
    let op = occt::full_round(&model.bodies[side1.body].shape, model.shape(side1), model.shape(side2), &centre)
        .map_err(|_| regen_error("FULL_ROUND_FILLET_FAILED"))?;
    replace_bodies(model, &id, vec![(side1.body, op)]);
    Ok(Value::UNDEFINED)
}

/// `@opFaceBlend`: a rolling ball blend of constant radius between two sets of faces meeting along edges, trimming
/// them to it; the sides' sheets become one. Each side's normal, flipped if asked, points towards the blend.
pub fn face_blend(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    if field(definition, "crossSection").as_str().is_some_and(|c| c != "ROLLING_BALL")
        || field(definition, "blendControlType").as_str().is_some_and(|c| c != "RADIUS")
        || values::opt_bool(definition, "asymmetric")?.unwrap_or(false)
        || field(definition, "crossSectionShape").as_str().is_some_and(|c| c != "CIRCULAR")
    {
        return Err(error("@opFaceBlend: only symmetric circular rolling ball blends of a radius are supported yet"));
    }
    let radius = values::length(field(definition, "radius"), "opFaceBlend radius")?;
    let side = |key: &str| -> R<Vec<EntityRef>> {
        Ok(query::evaluate(model, field(definition, key))?.into_iter().filter(|r| r.kind == EntityType::Face).collect())
    };
    let (side1, side2) = (side("side1")?, side("side2")?);
    if side1.is_empty() || side2.is_empty() {
        return Err(regen_error("FACE_BLEND_SELECT_FACES"));
    }
    let shapes = |refs: &[EntityRef]| -> Vec<Shape> { refs.iter().map(|r| model.shape(*r).clone()).collect() };
    let reverse1 = !values::opt_bool(definition, "flipSide1Normal")?.unwrap_or(false);
    let reverse2 = !values::opt_bool(definition, "flipSide2Normal")?.unwrap_or(false);
    let op = occt::face_blend(&shapes(&side1), reverse1, &shapes(&side2), reverse2, radius)
        .map_err(|_| regen_error("FACE_BLEND_FAILED"))?;
    // The sides' bodies give way to the blended sheet, which keeps the first's identity.
    let first = model.bodies[side1[0].body].clone();
    let mut bodies: Vec<usize> = side1.iter().chain(&side2).map(|r| r.body).collect();
    bodies.sort_unstable();
    bodies.dedup();
    let info = crate::model::Info::new(id);
    let sheets = occt::connected_shells(&op.shape()).map_err(crate::occt_error)?;
    model.remove_bodies(&bodies);
    for (i, sheet) in sheets.into_iter().enumerate() {
        let tid = (i == 0).then_some(first.tid);
        let new = crate::model::NewBody {
            tid,
            body_type: crate::model::BodyType::Sheet,
            ..crate::model::NewBody::replacing(&first, sheet)
        };
        model.add_body(new, &crate::model::Origins::default(), &info, &|_| None);
    }
    Ok(Value::UNDEFINED)
}

/// A variable fillet: the radius at each end of each edge is set at its vertex (`vertexSettings`), the fillet's
/// radius elsewhere, and goes from one to the other smoothly (level at both ends) or linearly.
fn variable_fillet(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    if field(definition, "pointOnEdgeSettings").as_array().is_some_and(|a| !a.is_empty()) {
        return Err(error("@opFillet: radii at points on edges aren't supported yet"));
    }
    let radius = values::length(field(definition, "radius"), "opFillet radius")?;
    let smooth = values::opt_bool(definition, "smoothTransition")?.unwrap_or(false);
    let mut at_vertex: Vec<(occt::Vec3, f64)> = Vec::new();
    for setting in field(definition, "vertexSettings").as_array().map(|a| a.to_vec()).unwrap_or_default() {
        let r = values::length(field(&setting, "vertexRadius"), "opFillet vertexRadius")?;
        for v in query::evaluate(model, field(&setting, "vertex"))? {
            if let Ok(p) = model.shape(v).vertex_point() {
                at_vertex.push((p, r));
            }
        }
    }
    let radius_at = |p: occt::Vec3| {
        at_vertex.iter().find(|(q, _)| values::norm(values::sub(*q, p)) < 1e-9).map(|(_, r)| *r).unwrap_or(radius)
    };
    let groups = blend_edges(model, definition)?;
    if groups.is_empty() {
        return Err(regen_error("FILLET_SELECT_EDGES"));
    }
    let mut results = Vec::new();
    for (b, edges) in groups {
        let (mut p0, mut r0, mut r1) = (Vec::new(), Vec::new(), Vec::new());
        for edge in &edges {
            let ends = edge.edge_end_vertices().unwrap_or_default();
            let points: Vec<occt::Vec3> = ends.iter().filter_map(|v| v.vertex_point().ok()).collect();
            let (a, z) = match points.as_slice() {
                [a, z, ..] => (*a, *z),
                [a] => (*a, *a),
                [] => return Err(regen_error("FILLET_FAILED")),
            };
            p0.push(a);
            r0.push(radius_at(a));
            r1.push(radius_at(z));
        }
        let op = occt::variable_fillet(&model.bodies[b].shape, &edges, &p0, &r0, &r1, smooth)
            .map_err(|_| regen_error("FILLET_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

/// A partial fillet: what a full fillet would remove, only where a bound keeps it. Each bound is a plane square to
/// its edge at a parameter along it. The fillet ends square.
fn partial_fillet(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let radius = values::length(field(definition, "radius"), "opFillet radius")?;
    let mut planes: Vec<occt::Frame> = Vec::new();
    for bound in field(definition, "partialFilletBounds").as_array().map(|a| a.to_vec()).unwrap_or_default() {
        let edges = query::evaluate(model, field(&bound, "boundaryEdge"))?;
        let edge = edges.iter().find(|r| r.kind == EntityType::Edge).ok_or_else(|| regen_error("FILLET_FAILED"))?;
        let t = field(&bound, "boundaryParameter").as_number().ok_or_else(|| error("@opFillet: boundaryParameter"))?;
        let flipped = field(&bound, "isFlipped").as_bool() == Some(true);
        let at = model.shape(*edge).edge_tangent(t, false).map_err(|_| regen_error("FILLET_FAILED"))?;
        let normal = if flipped { values::scale(at.direction, -1.0) } else { at.direction };
        planes.push(occt::Frame { origin: at.origin, normal, x: values::canonical_x(normal) });
    }
    let groups = blend_edges(model, definition)?;
    if groups.is_empty() {
        return Err(regen_error("FILLET_SELECT_EDGES"));
    }
    let mut results = Vec::new();
    for (b, edges) in groups {
        let body = model.bodies[b].shape.clone();
        let full = occt::fillet(&body, &edges, radius).map_err(|_| regen_error("FILLET_FAILED"))?;
        let removed =
            occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(&body), &[full.shape()], false)
                .map_err(|_| regen_error("FILLET_FAILED"))?
                .shape();
        // Each bound keeps the removed material on the side its edge comes from (flipped, the side it goes to);
        // the fillet is where any bound keeps it. (Batch 010, c003: bounds at a quarter and three quarters fillet
        // the two outer quarters.)
        let bounds = body.bounding_box(false).map_err(crate::occt_error)?;
        let size = values::norm(values::sub(bounds.max, bounds.min)) * 4.0 + 1.0;
        let removed = removed.sub_shapes(ShapeKind::Solid);
        let mut pieces = Vec::new();
        for plane in &planes {
            let cutter = occt::plane_face(*plane, -size, size, -size, size).map_err(crate::occt_error)?;
            for piece in &removed {
                let split = occt::split(std::slice::from_ref(piece), std::slice::from_ref(&cutter))
                    .map_err(crate::occt_error)?;
                for part in split.shape().sub_shapes(ShapeKind::Solid) {
                    let centre = part.centroid().map_err(crate::occt_error)?;
                    if values::dot(values::sub(centre, plane.origin), plane.normal) < 0.0 {
                        pieces.push(part);
                    }
                }
            }
        }
        if pieces.is_empty() {
            return Err(regen_error("FILLET_FAILED"));
        }
        let op = occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(&body), &pieces, true)
            .map_err(|_| regen_error("FILLET_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}
