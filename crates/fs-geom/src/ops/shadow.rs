//! `@opSplitBySelfShadow`: bodies' faces split where, seen along a direction, they turn from visible to invisible
//! (batch 093). Curved faces are split along their silhouettes, where their normal is square to the view; faces in
//! front are split where outlines cast shadows on them, the given bodies shadowing each other (and no others), and
//! pieces left on the same side merged again; the new edges are split where they're deepest or shallowest along
//! the view, as Onshape's are. A face is visible if it faces the viewer and nothing given lies between.

use super::owner_bodies;
use crate::model::{EntityType, FeatureId, Model, Tid};
use crate::values::{self, field};
use crate::{occt_error, query};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use fs_interp::value::ValueMap;
use occt::{Frame, Shape, ShapeKind, SurfaceType, Vec3};

/// Where a face's normal is this close to square to the view, it's edge-on: invisible.
const EDGE_ON: f64 = 1e-7;

pub fn split_by_self_shadow(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let bodies = owner_bodies(model, field(definition, "bodies"))?;
    let view =
        values::normalize(values::vec3(field(definition, "viewDirection"), "opSplitBySelfShadow viewDirection")?)
            .ok_or_else(|| error("@opSplitBySelfShadow: the view direction is zero"))?;
    let boxes: Vec<occt::Box3> =
        bodies.iter().filter_map(|&b| model.bodies[b].shape.bounding_box(false).ok()).filter(|b| !b.empty).collect();
    let Some(first) = boxes.first() else { return Ok(result(Vec::new(), Vec::new())) };
    let (mut low, mut high) = (first.min, first.max);
    for b in &boxes {
        low = Vec3::new(low.x.min(b.min.x), low.y.min(b.min.y), low.z.min(b.min.z));
        high = Vec3::new(high.x.max(b.max.x), high.y.max(b.max.y), high.z.max(b.max.z));
    }
    let reach = values::norm(values::sub(high, low)) * 2.0 + 1.0;
    let originals: Vec<Tid> = bodies.iter().flat_map(|&b| model.bodies[b].faces.iter().map(|f| f.tid)).collect();
    // Replacing bodies moves them in the model's list: they're found again by transient id after each step.
    let body_tids: Vec<Tid> = bodies.iter().map(|&b| model.bodies[b].tid).collect();
    let find = |model: &Model| -> Vec<usize> {
        body_tids.iter().filter_map(|t| model.bodies.iter().position(|body| body.tid == *t)).collect()
    };

    // Silhouettes: each curved face cut where its normal turns square to the view.
    let mut results = Vec::new();
    for &b in &bodies {
        let faces: Vec<Shape> = model.bodies[b].faces.iter().map(|f| f.shape.clone()).collect();
        let (mut tools, mut owners) = (Vec::new(), Vec::new());
        for (i, face) in faces.iter().enumerate() {
            for tool in silhouette_tools(face, view, reach)? {
                tools.push(tool);
                owners.push(i as i32);
            }
        }
        if !tools.is_empty() {
            let op = occt::split_faces_paired(&model.bodies[b].shape, &faces, &tools, &owners).map_err(occt_error)?;
            results.push((b, op));
        }
    }
    if !results.is_empty() {
        super::replace_bodies(model, &id, results);
    }

    let bodies = find(model);
    // Shadows: a front face passes into shadow where it crosses the wall an outline (an edge between a face in
    // front and one not) sweeps away from the viewer.
    let mut walls = Vec::new();
    let mut front: Vec<(usize, Shape)> = Vec::new();
    for &b in &bodies {
        let faces: Vec<Shape> = model.bodies[b].faces.iter().map(|f| f.shape.clone()).collect();
        let mut is_front = Vec::new();
        for face in &faces {
            is_front.push(facing(face, view)? < -EDGE_ON);
        }
        front.extend(faces.iter().zip(&is_front).filter(|(_, f)| **f).map(|(face, _)| (b, face.clone())));
        for (edge, adjacent) in edge_faces(&model.bodies[b].shape, &faces) {
            if adjacent.len() == 2 && adjacent.iter().filter(|&&i| is_front[i]).count() == 1 {
                walls.push(occt::prism(&edge, values::scale(view, reach)).map_err(occt_error)?.shape());
            }
        }
    }
    if !walls.is_empty() {
        let owners = vec![-1; walls.len()];
        let mut results = Vec::new();
        for &b in &bodies {
            let faces: Vec<Shape> = front.iter().filter(|(fb, _)| *fb == b).map(|(_, f)| f.clone()).collect();
            if !faces.is_empty() {
                let op =
                    occt::split_faces_paired(&model.bodies[b].shape, &faces, &walls, &owners).map_err(occt_error)?;
                results.push((b, op));
            }
        }
        if !results.is_empty() {
            super::replace_bodies(model, &id, results);
        }
    }

    let bodies = find(model);
    // Pieces of one face on the same side are one face again: where an outline's shadow falls in shadow anyway,
    // and across a cylinder's or a cone's seam.
    let mut results = Vec::new();
    let all = all_faces(model, &bodies);
    for &b in &bodies {
        let faces: Vec<Shape> = model.bodies[b].faces.iter().map(|f| f.shape.clone()).collect();
        let sources: Vec<Option<Tid>> = (0..faces.len()).map(|index| source(model, b, index, &originals)).collect();
        let mut seen = Vec::new();
        for face in &faces {
            seen.push(sees(face, view, &all)?);
        }
        let removable: Vec<Shape> = edge_faces(&model.bodies[b].shape, &faces)
            .into_iter()
            .filter(|(_, adjacent)| {
                let &[i, j] = adjacent.as_slice() else { return false };
                sources[i].is_some() && sources[i] == sources[j] && seen[i] == seen[j]
            })
            .map(|(edge, _)| edge)
            .collect();
        if !removable.is_empty() {
            let op = occt::merge_faces_across(&model.bodies[b].shape, &removable).map_err(occt_error)?;
            results.push((b, op));
        }
    }
    if !results.is_empty() {
        super::replace_bodies(model, &id, results);
    }

    let bodies = find(model);
    // The new edges, split where they turn back along the view.
    let mut results = Vec::new();
    for &b in &bodies {
        let mut vertices = Vec::new();
        for (index, edge) in model.bodies[b].edges.iter().enumerate() {
            let r = crate::model::EntityRef { body: b, kind: EntityType::Edge, index };
            if !model.info(r).is_created_by(&id) {
                continue;
            }
            for p in depth_turns(&edge.shape, view)? {
                vertices.push(occt::vertex(p).map_err(occt_error)?);
            }
        }
        if !vertices.is_empty() {
            results
                .push((b, occt::split(std::slice::from_ref(&model.bodies[b].shape), &vertices).map_err(occt_error)?));
        }
    }
    if !results.is_empty() {
        super::replace_bodies(model, &id, results);
    }

    let bodies = find(model);
    // Which faces are visible; the split ones, by which side of the split they're on.
    let all = all_faces(model, &bodies);
    let (mut visible, mut invisible) = (Vec::new(), Vec::new());
    let mut pieces: Vec<(Tid, Tid, bool)> = Vec::new();
    for &b in &bodies {
        for index in 0..model.bodies[b].faces.len() {
            let face = &model.bodies[b].faces[index];
            let seen = sees(&face.shape, view, &all)?;
            if seen {
                visible.push(face.tid);
            } else {
                invisible.push(face.tid);
            }
            if let Some(source) = source(model, b, index, &originals) {
                pieces.push((source, face.tid, !seen));
            }
        }
    }
    let mut sides = Vec::new();
    for (source, tid, back) in &pieces {
        if pieces.iter().filter(|(s, _, _)| s == source).count() > 1 {
            sides.push((id.clone(), *tid, *back));
        }
    }
    std::rc::Rc::make_mut(&mut model.splits).extend(sides);
    Ok(result(visible, invisible))
}

fn all_faces(model: &Model, bodies: &[usize]) -> Vec<Shape> {
    bodies.iter().flat_map(|&b| model.bodies[b].faces.iter().map(|f| f.shape.clone())).collect()
}

/// The face, of those the bodies had before, that a face is or is a piece of.
fn source(model: &Model, b: usize, index: usize, originals: &[Tid]) -> Option<Tid> {
    let entity = model.entity(crate::model::EntityRef { body: b, kind: EntityType::Face, index }).expect("a face");
    if originals.contains(&entity.tid) {
        return Some(entity.tid);
    }
    entity.ancestry.identity.iter().copied().find(|t| originals.contains(t))
}

/// Each edge of a body with the faces (indices into `faces`) it bounds, each once.
fn edge_faces(body: &Shape, faces: &[Shape]) -> Vec<(Shape, Vec<usize>)> {
    let mut out: Vec<(Shape, Vec<usize>)> = Vec::new();
    for edge in body.sub_shapes(ShapeKind::Edge) {
        if out.iter().any(|(e, _)| e.is_same(&edge)) {
            continue;
        }
        let adjacent: Vec<usize> = faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(&edge)))
            .map(|(i, _)| i)
            .collect();
        out.push((edge, adjacent));
    }
    out
}

fn result(visible: Vec<Tid>, invisible: Vec<Tid>) -> Value {
    let ids = |tids: Vec<Tid>| Value::array(tids.into_iter().map(|t| Value::string(query::tid_string(t))).collect());
    let mut entries = ValueMap::new();
    entries.insert(Value::string("visibleFaces"), ids(visible));
    entries.insert(Value::string("invisibleFaces"), ids(invisible));
    Value::map(entries)
}

/// The cosine between a face's outward normal at a point inside it and the view: negative where it faces the
/// viewer.
fn facing(face: &Shape, view: Vec3) -> R<f64> {
    let p = face.face_inner_point().map_err(occt_error)?;
    let (fu, fv) = face.face_fraction(p).map_err(occt_error)?;
    Ok(values::dot(face.face_tangent_plane(fu, fv).map_err(occt_error)?.normal, view))
}

/// Whether a face is visible: it faces the viewer, and nothing of `all` lies between it and them.
fn sees(face: &Shape, view: Vec3, all: &[Shape]) -> R<bool> {
    if facing(face, view)? >= -EDGE_ON {
        return Ok(false);
    }
    let p = face.face_inner_point().map_err(occt_error)?;
    let back = values::scale(view, -1.0);
    for other in all {
        if other.line_crossings(p, back).map_err(occt_error)?.iter().any(|&d| d > 1e-6) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Faces cutting a curved face along its silhouette: a plane through a cylinder's axis or a sphere's centre square
/// to the view, half planes from a cone's axis through its two silhouette lines.
fn silhouette_tools(face: &Shape, view: Vec3, reach: f64) -> R<Vec<Shape>> {
    let kind = face.surface_type().map_err(occt_error)?;
    let plane = |origin: Vec3, normal: Vec3, x: Vec3, v0: f64| {
        occt::plane_face(Frame { origin, normal, x }, -reach, reach, v0, reach).map_err(occt_error)
    };
    match kind {
        SurfaceType::Plane => Ok(Vec::new()),
        SurfaceType::Cylinder => {
            let s = face.surface_definition().map_err(occt_error)?;
            let axis = s.frame.normal;
            let across = values::sub(view, values::scale(axis, values::dot(view, axis)));
            match values::normalize(across) {
                Some(normal) if values::norm(across) > 1e-9 => Ok(vec![plane(s.frame.origin, normal, axis, -reach)?]),
                _ => Ok(Vec::new()),
            }
        }
        SurfaceType::Sphere => {
            let s = face.surface_definition().map_err(occt_error)?;
            Ok(vec![plane(s.frame.origin, view, perpendicular(view), -reach)?])
        }
        SurfaceType::Cone => {
            let s = face.surface_definition().map_err(occt_error)?;
            let apex = s.frame.origin;
            // The way the cone widens.
            let inside = face.face_inner_point().map_err(occt_error)?;
            let mut axis = s.frame.normal;
            if values::dot(values::sub(inside, apex), axis) < 0.0 {
                axis = values::scale(axis, -1.0);
            }
            let across = values::sub(view, values::scale(axis, values::dot(view, axis)));
            let width = values::norm(across);
            if width < 1e-9 {
                return Ok(Vec::new());
            }
            // The outward normal at radial direction r is cos α r − sin α axis: square to the view where
            // r · across = tan α (axis · view).
            let e1 = values::scale(across, 1.0 / width);
            let e2 = values::cross(axis, e1);
            let c = s.radius1.tan() * values::dot(axis, view) / width;
            if c.abs() >= 1.0 - 1e-12 {
                return Ok(Vec::new());
            }
            let sine = (1.0 - c * c).sqrt();
            let mut tools = Vec::new();
            for sign in [1.0, -1.0] {
                let radial = values::add(values::scale(e1, c), values::scale(e2, sign * sine));
                let normal = values::normalize(values::cross(axis, radial)).expect("radial is square to the axis");
                tools.push(plane(apex, normal, axis, 0.0)?);
            }
            Ok(tools)
        }
        other => Err(error(format!("@opSplitBySelfShadow: silhouettes on {other:?} faces aren't supported yet"))),
    }
}

fn perpendicular(v: Vec3) -> Vec3 {
    let seed = if v.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
    values::normalize(values::cross(v, seed)).expect("not parallel")
}

/// The points inside an edge where its depth along the view turns.
fn depth_turns(edge: &Shape, view: Vec3) -> R<Vec<Vec3>> {
    const SAMPLES: usize = 64;
    let slope =
        |t: f64| -> R<f64> { Ok(values::dot(edge.edge_tangent(t, false).map_err(occt_error)?.direction, view)) };
    let mut turns = Vec::new();
    let mut previous = (0.0, slope(0.0)?);
    for i in 1..=SAMPLES {
        let t = i as f64 / SAMPLES as f64;
        let s = slope(t)?;
        if previous.1.abs() > 1e-9 && s.abs() > 1e-9 && previous.1.signum() != s.signum() {
            let (mut a, mut b) = (previous.0, t);
            let sa = previous.1;
            for _ in 0..60 {
                let m = 0.5 * (a + b);
                if slope(m)?.signum() == sa.signum() {
                    a = m;
                } else {
                    b = m;
                }
            }
            let m = 0.5 * (a + b);
            if m > 1e-6 && m < 1.0 - 1e-6 {
                turns.push(edge.edge_tangent(m, false).map_err(occt_error)?.origin);
            }
        }
        previous = (t, s);
    }
    Ok(turns)
}
