//! `@opSplitPart`: splitting bodies with a plane, a face or a sheet.

use super::owner_bodies;
use crate::model::{EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, field};
use crate::{occt_error, query};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Box3, Frame, Shape, ShapeKind, SurfaceType, Vec3};

/// A planar face on `frame`'s plane big enough to cut right through `bounds`: a stand-in for the infinite plane.
fn plane_through(frame: Frame, bounds: &Box3) -> R<Shape> {
    let centre = values::scale(values::add(bounds.min, bounds.max), 0.5);
    let size = values::norm(values::sub(bounds.max, bounds.min)) * 4.0 + 1.0;
    let offset = values::dot(values::sub(centre, frame.origin), frame.normal);
    let origin = values::sub(centre, values::scale(frame.normal, offset));
    occt::plane_face(Frame { origin, ..frame }, -size, size, -size, size).map_err(occt_error)
}

fn union_box(boxes: impl Iterator<Item = Box3>) -> Option<Box3> {
    boxes.filter(|b| !b.empty).reduce(|a, b| Box3 {
        min: Vec3::new(a.min.x.min(b.min.x), a.min.y.min(b.min.y), a.min.z.min(b.min.z)),
        max: Vec3::new(a.max.x.max(b.max.x), a.max.y.max(b.max.y), a.max.z.max(b.max.z)),
        empty: false,
    })
}

pub fn split_part(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let targets = owner_bodies(model, field(definition, "targets"))?;
    let keep_tools = values::opt_bool(definition, "keepTools")?.unwrap_or(false);
    let use_trimmed = values::opt_bool(definition, "useTrimmed")?.unwrap_or(false);
    let keep = field(definition, "keepType").as_str().unwrap_or("KEEP_ALL").to_string();
    let bounds = union_box(targets.iter().filter_map(|&b| model.bodies[b].shape.bounding_box(false).ok()))
        .ok_or_else(|| error("@opSplitPart: nothing to split"))?;

    // The tool: a plane, one face (extended to its plane if planar, unless trimmed), or sheet bodies.
    let tool = field(definition, "tool");
    let mut delete_tools = Vec::new();
    let (tools, plane): (Vec<Shape>, Option<Frame>) = if tool.get("normal").is_some() {
        let frame = values::plane(tool, "opSplitPart tool")?;
        (vec![plane_through(frame, &bounds)?], Some(frame))
    } else {
        let found = query::evaluate(model, tool)?;
        let faces: Vec<_> = found.iter().filter(|r| r.kind == EntityType::Face).collect();
        if faces.len() == 1 && found.len() == 1 {
            let face = model.shape(*faces[0]).clone();
            match face.surface_type().ok() {
                Some(SurfaceType::Plane) => {
                    let frame = face.face_plane().map_err(occt_error)?;
                    let shape = if use_trimmed { face } else { plane_through(frame, &bounds)? };
                    (vec![shape], Some(frame))
                }
                _ => (vec![face], None),
            }
        } else {
            let bodies = owner_bodies(model, tool)?;
            if !keep_tools {
                delete_tools = bodies.iter().copied().filter(|b| !model.bodies[*b].construction).collect();
            }
            let shapes: Vec<Shape> = bodies.iter().map(|&b| model.bodies[b].shape.clone()).collect();
            let plane = match shapes.as_slice() {
                [one] if one.sub_shapes(ShapeKind::Face).len() == 1 => {
                    one.sub_shapes(ShapeKind::Face)[0].face_plane().ok()
                }
                _ => None,
            };
            (shapes, plane)
        }
    };
    if keep != "KEEP_ALL" && plane.is_none() {
        return Err(error("@opSplitPart: keeping one side needs a planar tool for now"));
    }

    let default = Info::new(id);
    let mut results = Vec::new();
    for &t in &targets {
        let body = model.bodies[t].clone();
        let op = occt::split(std::slice::from_ref(&body.shape), &tools).map_err(occt_error)?;
        let origins = std::rc::Rc::new(Origins::of_operation(&op, &[&body]));
        let kind = match body.body_type {
            crate::model::BodyType::Solid => ShapeKind::Solid,
            crate::model::BodyType::Sheet => ShapeKind::Face,
            crate::model::BodyType::Wire => ShapeKind::Edge,
            _ => continue,
        };
        let mut first = true;
        for piece in op.shape().sub_shapes(kind) {
            // Which side of the tool the piece is on, for qSplitBy (in front: along the tool's normal).
            let mut back = None;
            if let Some(frame) = plane {
                let centre = piece.centroid().map_err(occt_error)?;
                let front = values::dot(values::sub(centre, frame.origin), frame.normal) > 0.0;
                if (keep == "KEEP_FRONT" && !front) || (keep == "KEEP_BACK" && front) {
                    continue;
                }
                back = Some(!front);
            }
            let tid = first.then_some(body.tid);
            first = false;
            results.push((NewBody { tid, ..NewBody::replacing(&body, piece) }, origins.clone(), back));
        }
    }
    let mut remove = targets.clone();
    remove.extend(delete_tools);
    model.remove_bodies(&remove);
    let mut sides = Vec::new();
    for (new, origins, back) in results {
        let b = model.add_body(new, &origins, &default, &|_| None);
        if let Some(back) = back {
            sides.push((default.created_by.clone(), model.bodies[b].tid, back));
        }
    }
    std::rc::Rc::make_mut(&mut model.splits).extend(sides);
    Ok(Value::UNDEFINED)
}

/// `@opSplitFace`: splits faces where tools cross them, leaving the rest of their bodies as they are. Edge tools
/// are swept along the projection direction (for faces of planes, their normal by default) into cutting sheets;
/// plane tools are infinite. Returns the edges the split made (`splittingEdges`).
/// `@opSplitEdges` at parameters: each edge split at the points its own array of parameters gives (by arc length
/// unless `arcLengthParameterization` is false), by splitting its body with vertices there.
pub fn split_edges(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    if !field(definition, "splittingSurface").is_undefined() {
        return Err(error("@opSplitEdges: splitting by a surface isn't supported yet"));
    }
    let edges: Vec<crate::model::EntityRef> = query::evaluate(model, field(definition, "edges"))?
        .into_iter()
        .filter(|r| r.kind == EntityType::Edge)
        .collect();
    if edges.is_empty() {
        return Err(crate::regen_error("SPLIT_EDGE_SELECT_EDGES"));
    }
    let parameters = field(definition, "parameters")
        .as_array()
        .ok_or_else(|| error("@opSplitEdges: parameters must be an array of arrays"))?
        .clone();
    let arc_length = values::opt_bool(definition, "arcLengthParameterization")?.unwrap_or(true);
    let mut by_body: Vec<(usize, Vec<Shape>)> = Vec::new();
    let mut split_tids = Vec::new();
    for (i, r) in edges.iter().enumerate() {
        let Some(list) = parameters.get(i) else { break };
        let ts = values::numbers(list, "opSplitEdges parameters")?;
        let edge = model.shape(*r).clone();
        let length = edge.length().map_err(occt_error)?;
        for t in ts {
            if !(-1e-12..=1.0 + 1e-12).contains(&t) {
                return Err(crate::regen_error("SPLIT_EDGE_PARAMETER_BOUND"));
            }
            // At an end there's nothing to split.
            if t * length < 1e-8 || (1.0 - t) * length < 1e-8 {
                continue;
            }
            let point = edge.edge_tangent(t, arc_length).map_err(occt_error)?.origin;
            split_tids.push(model.tid(*r));
            let vertex = occt::vertex(point).map_err(occt_error)?;
            match by_body.iter_mut().find(|(b, _)| *b == r.body) {
                Some((_, vertices)) => vertices.push(vertex),
                None => by_body.push((r.body, vec![vertex])),
            }
        }
    }
    let mut results = Vec::new();
    for (b, vertices) in by_body {
        let op = occt::split(std::slice::from_ref(&model.bodies[b].shape), &vertices)
            .map_err(|_| crate::regen_error("SPLIT_EDGE_FAILED"))?;
        results.push((b, op));
    }
    super::replace_bodies(model, &id, results);
    // The pieces of each split edge, for `qSplitBy` (std finds the piece a partial flange goes on by a point on it).
    let mut pieces = Vec::new();
    for r in model.everything(Some(EntityType::Edge)) {
        let entity = model.entity(r).expect("an edge");
        if split_tids.contains(&entity.tid) || entity.ancestry.identity.iter().any(|t| split_tids.contains(t)) {
            pieces.push((id.clone(), entity.tid, false));
        }
    }
    std::rc::Rc::make_mut(&mut model.splits).extend(pieces);
    Ok(Value::UNDEFINED)
}

pub fn split_face(model: &mut Model, id: FeatureId, definition: &Value, tags: &values::Tags) -> R<Value> {
    let groups = super::by_body(model, field(definition, "faceTargets"), EntityType::Face)?;
    if groups.is_empty() {
        return Err(crate::regen_error("SPLIT_FACE_SELECT_TARGETS"));
    }
    let optional = |key: &str| -> R<Vec<crate::model::EntityRef>> {
        match field(definition, key) {
            v if v.is_undefined() => Ok(Vec::new()),
            v => query::evaluate(model, v),
        }
    };
    let bounds = union_box(model.bodies.iter().filter_map(|b| b.shape.bounding_box(false).ok()))
        .ok_or_else(|| error("@opSplitFace: nothing to split"))?;
    let reach = values::norm(values::sub(bounds.max, bounds.min)) * 2.0 + 1.0;
    let direction = match field(definition, "direction") {
        v if v.is_undefined() => None,
        v => Some(values::normalize(values::vec3(v, "opSplitFace direction")?).ok_or_else(|| error("zero direction"))?),
    };
    let mut tools: Vec<Shape> = Vec::new();
    for r in optional("edgeTools")? {
        let edge = model.shape(r).clone();
        let d = match direction {
            Some(d) => d,
            // Normal to the target: the target's plane normal.
            None => groups
                .iter()
                .flat_map(|(_, faces)| faces.iter())
                .find_map(|f| f.face_plane().ok())
                .map(|p| p.normal)
                .ok_or_else(|| error("@opSplitFace: projecting edges onto curved faces isn't supported yet"))?,
        };
        let back = occt::transform(&edge, &super::translation(values::scale(d, -reach))).map_err(occt_error)?.shape();
        tools.push(occt::prism(&back, values::scale(d, 2.0 * reach)).map_err(occt_error)?.shape());
    }
    for r in optional("planeTools")? {
        let plane = model.shape(r).face_plane().map_err(|_| crate::regen_error("SPLIT_FACE_FAILED"))?;
        tools.push(plane_through(plane, &bounds)?);
    }
    // Tool faces, with their transient ids: the new edges they make are tracked back to them.
    let mut tool_faces: Vec<(crate::model::Tid, Shape)> = Vec::new();
    for r in optional("faceTools")? {
        tools.push(model.shape(r).clone());
        if r.kind == EntityType::Face {
            tool_faces.push((model.tid(r), model.shape(r).clone()));
        }
    }
    let body_tools = optional("bodyTools")?;
    for r in &body_tools {
        tools.push(model.bodies[r.body].shape.clone());
    }
    if tools.is_empty() {
        return Err(crate::regen_error("SPLIT_FACE_SELECT_TOOLS"));
    }
    let mut groups = groups;
    // With mutual imprint and only body tools, the tools' faces are split by the targets' bodies too.
    let mutual = values::opt_bool(definition, "mutualImprint")?.unwrap_or(false);
    let mut tool_groups: Vec<(usize, Vec<Shape>, Vec<Shape>)> = Vec::new();
    if mutual && !body_tools.is_empty() && tools.len() == body_tools.len() {
        let target_shapes: Vec<Shape> = groups.iter().map(|(b, _)| model.bodies[*b].shape.clone()).collect();
        let mut seen = Vec::new();
        for r in &body_tools {
            if seen.contains(&r.body) || groups.iter().any(|(b, _)| *b == r.body) {
                continue;
            }
            seen.push(r.body);
            let faces: Vec<Shape> = model.bodies[r.body].faces.iter().map(|f| f.shape.clone()).collect();
            if !faces.is_empty() {
                tool_groups.push((r.body, faces, target_shapes.clone()));
            }
        }
    }
    let mut results = Vec::new();
    let mut split_tids = Vec::new();
    // The faces split, by transient id, and the tools to tell their pieces' sides by.
    let mut sided: Vec<(Vec<crate::model::Tid>, Vec<Shape>)> = Vec::new();
    let face_tids = |model: &Model, b: usize, faces: &[Shape]| -> Vec<crate::model::Tid> {
        model.bodies[b].faces.iter().filter(|f| faces.iter().any(|g| g.is_same(&f.shape))).map(|f| f.tid).collect()
    };
    for (b, faces, cutters) in tool_groups {
        if let Ok(op) = occt::split_faces(&model.bodies[b].shape, &faces, &cutters) {
            split_tids.push(model.bodies[b].tid);
            sided.push((face_tids(model, b, &faces), cutters));
            results.push((b, op));
        }
    }
    for (b, faces) in groups.drain(..) {
        let op = occt::split_faces(&model.bodies[b].shape, &faces, &tools)
            .map_err(|_| crate::regen_error("SPLIT_FACE_FAILED"))?;
        split_tids.push(model.bodies[b].tid);
        sided.push((face_tids(model, b, &faces), tools.clone()));
        results.push((b, op));
    }
    super::replace_bodies(model, &id, results);
    // Which side of the tools each piece of a split face is on. Faces the tools didn't cut aren't split: jogs
    // split their faces up front and then expect the bends' own splits to split nothing (batch 013).
    let mut sides = Vec::new();
    for (tids, cutters) in &sided {
        let mut pieces: Vec<(crate::model::Tid, crate::model::Tid, Shape)> = Vec::new();
        for r in model.everything(Some(EntityType::Face)) {
            let entity = model.entity(r).expect("a face");
            let source = if tids.contains(&entity.tid) {
                entity.tid
            } else if let Some(t) = entity.ancestry.identity.iter().find(|t| tids.contains(t)) {
                *t
            } else {
                continue;
            };
            pieces.push((source, entity.tid, entity.shape.clone()));
        }
        for (source, tid, shape) in &pieces {
            if pieces.iter().filter(|(s, _, _)| s == source).count() < 2 {
                continue;
            }
            if let Some(back) = behind(shape, cutters) {
                sides.push((id.clone(), *tid, back));
            }
        }
    }
    std::rc::Rc::make_mut(&mut model.splits).extend(sides);
    if !values::opt_bool(definition, "keepToolSurfaces")?.unwrap_or(true) {
        let mut consumed: Vec<usize> = body_tools.iter().map(|r| r.body).collect();
        consumed.dedup();
        model.remove_bodies(&consumed);
    }
    // The new edges depend on the tool faces they lie on (std tracks them from the tools, partial dependency).
    // With `ownExistingImprints`, so do edges already there that lie on the tools.
    let own_existing = values::opt_bool(definition, "ownExistingImprints")?.unwrap_or(false);
    if !tool_faces.is_empty() {
        for r in model.everything(Some(EntityType::Edge)) {
            if !split_tids.contains(&model.bodies[r.body].tid) || !(own_existing || model.info(r).is_created_by(&id)) {
                continue;
            }
            let shape = model.shape(r).clone();
            let samples: Vec<Vec3> =
                [0.0, 0.5, 1.0].iter().filter_map(|&t| shape.edge_tangent(t, false).ok().map(|a| a.origin)).collect();
            let from: Vec<crate::model::Tid> = tool_faces
                .iter()
                .filter(|(_, face)| {
                    let surface = face.face_plane().ok();
                    samples.iter().all(|p| match surface {
                        Some(plane) => values::dot(values::sub(*p, plane.origin), plane.normal).abs() < 1e-7,
                        None => occt::vertex(*p).and_then(|v| v.distance(face)).is_ok_and(|d| d.distance < 1e-7),
                    })
                })
                .map(|(tid, _)| *tid)
                .collect();
            if !from.is_empty() {
                model.add_ancestors(r, &from);
            }
        }
    }
    // The new edges: those this operation made on the split bodies.
    let mut edges = Vec::new();
    for r in model.everything(Some(EntityType::Edge)) {
        if split_tids.contains(&model.bodies[r.body].tid) && model.info(r).is_created_by(&id) {
            edges.push(tags.transient_query(&query::tid_string(model.tid(r))));
        }
    }
    Ok(values::map(vec![("splittingEdges", Value::array(edges))]))
}

/// Whether a face is behind the nearest of some tools: on the side opposite its normal there.
fn behind(face: &Shape, tools: &[Shape]) -> Option<bool> {
    let p = face.face_inner_point().ok()?;
    let point = occt::vertex(p).ok()?;
    let mut nearest: Option<(f64, Shape, Vec3)> = None;
    for tool in tools {
        for f in tool.sub_shapes(ShapeKind::Face) {
            if let Ok(d) = point.distance(&f)
                && nearest.as_ref().is_none_or(|(best, _, _)| d.distance < *best)
            {
                nearest = Some((d.distance, f, d.b));
            }
        }
    }
    let (_, f, q) = nearest?;
    let (fu, fv) = f.face_fraction(q).ok()?;
    let normal = f.face_tangent_plane(fu, fv).ok()?.normal;
    Some(values::dot(values::sub(p, q), normal) < 0.0)
}
