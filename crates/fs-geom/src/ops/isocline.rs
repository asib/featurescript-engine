//! Isoclines: the curves on faces where the tangent plane makes a given angle with a direction, like a draft
//! angle (`@opCreateIsocline`, `@opSplitByIsocline`). Spheres, and tori about an axis along the direction, have
//! them on planes square to the direction; other faces aren't supported yet. (Batch 007, c026–c027.)

use std::rc::Rc;

use super::{by_body, replace_bodies};
use crate::model::{BodyType, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::query::{self, tid_string};
use crate::values::{self, add, cross, dot, field, map, norm, normalize, scale, sub};
use crate::{occt_error, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Frame, Shape, Vec3};

/// A face's outward normal at a point on it.
fn normal_at(face: &Shape, p: Vec3) -> Option<Vec3> {
    let (fu, fv) = face.face_fraction(p).ok()?;
    face.face_tangent_plane(fu, fv).ok().map(|f| f.normal)
}

/// The planes on which a face's isoclines at `angle` from `direction` lie.
fn isocline_planes(face: &Shape, direction: Vec3, angle: f64) -> R<Vec<Frame>> {
    let s = face.surface_definition().map_err(occt_error)?;
    let p = face.face_inner_point().map_err(occt_error)?;
    let n = normal_at(face, p).ok_or_else(|| regen_error("ISOCLINE_NO_RESULT"))?;
    let target = angle.sin();
    let plane_at = |origin: Vec3| Frame { origin, normal: direction, x: values::canonical_x(direction) };
    match s.kind {
        // Sphere: the normal is (p − c)/R (or its opposite), so the isocline is where (p − c)·d = ±R sin a.
        3 => {
            let (c, radius) = (s.frame.origin, s.radius1);
            let sign = if dot(n, sub(p, c)) >= 0.0 { 1.0 } else { -1.0 };
            let h = sign * radius * target;
            Ok(if h.abs() < radius { vec![plane_at(add(c, scale(direction, h)))] } else { Vec::new() })
        }
        // Torus about an axis along the direction: the normal at tube angle θ rises at sin θ, so both circles at
        // height r sin θ = ±r sin a are isoclines.
        4 => {
            let (c, a, major, minor) = (s.frame.origin, s.frame.normal, s.radius1, s.radius2);
            if norm(cross(a, direction)) > 1e-9 {
                return Err(error("@opCreateIsocline: tori tilted to the direction aren't supported yet"));
            }
            let rel = sub(p, c);
            let radial = normalize(sub(rel, scale(a, dot(rel, a)))).ok_or_else(|| regen_error("ISOCLINE_NO_RESULT"))?;
            let tube = add(c, scale(radial, major));
            let sign = if dot(n, sub(p, tube)) >= 0.0 { 1.0 } else { -1.0 };
            let h = sign * minor * target * dot(a, direction).signum();
            Ok(if h.abs() < minor { vec![plane_at(add(c, scale(a, h)))] } else { Vec::new() })
        }
        // Planes, and cylinders and cones about the direction, have the same angle everywhere.
        0 => Ok(Vec::new()),
        1 | 2 if norm(cross(s.frame.normal, direction)) < 1e-9 => Ok(Vec::new()),
        _ => Err(error("@opCreateIsocline: isoclines of this kind of face aren't supported yet")),
    }
}

/// Large planar faces standing in for the isocline planes of some faces.
fn isocline_cutters(faces: &[Shape], direction: Vec3, angle: f64) -> R<Vec<Shape>> {
    let mut out = Vec::new();
    for face in faces {
        let bounds = face.bounding_box(false).map_err(occt_error)?;
        let size = norm(sub(bounds.max, bounds.min)) * 2.0 + 1e-3;
        for plane in isocline_planes(face, direction, angle)? {
            out.push(occt::plane_face(plane, -size, size, -size, size).map_err(occt_error)?);
        }
    }
    Ok(out)
}

fn direction_and_angle(definition: &Value) -> R<(Vec3, f64)> {
    let direction = normalize(values::vec3(field(definition, "direction"), "isocline direction")?)
        .ok_or_else(|| regen_error("ISOCLINE_SELECT_DIRECTION"))?;
    Ok((direction, values::angle(field(definition, "angle"), "isocline angle")?))
}

/// `@opCreateIsocline`: a wire body for each isocline.
pub fn create(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let (direction, angle) = direction_and_angle(definition)?;
    let faces: Vec<Shape> = query::evaluate(model, field(definition, "faces"))?
        .into_iter()
        .filter(|r| r.kind == EntityType::Face)
        .map(|r| model.shape(r).clone())
        .collect();
    let info = Info::new(id);
    for face in &faces {
        let cutters = isocline_cutters(std::slice::from_ref(face), direction, angle)?;
        if cutters.is_empty() {
            continue;
        }
        let edges = occt::section_edges(std::slice::from_ref(face), &cutters).map_err(occt_error)?;
        for wire in occt::connect_edges(&edges).map_err(occt_error)? {
            model.add_body(NewBody::new(wire, BodyType::Wire, info.clone()), &Origins::default(), &info, &|_| None);
        }
    }
    Ok(Value::UNDEFINED)
}

/// `@opSplitByIsocline`: the faces split along their isoclines. The pieces whose angle is at least the isocline
/// angle are in front of the split (`qSplitBy`), the steep ones behind. Returns transient ids of the steep and
/// other faces and the new edges.
pub fn split(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let (direction, angle) = direction_and_angle(definition)?;
    let groups = by_body(model, field(definition, "faces"), EntityType::Face)?;
    if groups.is_empty() {
        return Err(regen_error("ISOCLINE_SELECT_FACES"));
    }
    let mut results = Vec::new();
    let mut split_tids = Vec::new();
    let mut input_tids = Vec::new();
    for (b, faces) in groups {
        input_tids
            .extend(model.bodies[b].faces.iter().filter(|f| faces.iter().any(|g| g.is_same(&f.shape))).map(|f| f.tid));
        let cutters = isocline_cutters(&faces, direction, angle)?;
        if cutters.is_empty() {
            continue;
        }
        let op = occt::split_faces(&model.bodies[b].shape, &faces, &cutters)
            .map_err(|_| regen_error("ISOCLINE_NO_RESULT"))?;
        split_tids.push(model.bodies[b].tid);
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    let (mut steep, mut not_steep, mut boundary) = (Vec::new(), Vec::new(), Vec::new());
    let mut sides = Vec::new();
    for r in model.everything(Some(EntityType::Face)) {
        let entity = model.entity(r).expect("a face");
        let from_input =
            input_tids.contains(&entity.tid) || entity.ancestry.identity.iter().any(|t| input_tids.contains(t));
        if !from_input {
            continue;
        }
        let Some(n) = entity.shape.face_inner_point().ok().and_then(|p| normal_at(&entity.shape, p)) else { continue };
        let is_steep = dot(n, direction).asin() < angle;
        let transient = Value::string(tid_string(entity.tid));
        if is_steep {
            steep.push(transient)
        } else {
            not_steep.push(transient)
        }
        if !input_tids.contains(&entity.tid) {
            sides.push((id.clone(), entity.tid, is_steep));
        }
    }
    Rc::make_mut(&mut model.splits).extend(sides);
    for r in model.everything(Some(EntityType::Edge)) {
        if split_tids.contains(&model.bodies[r.body].tid) && model.info(r).is_created_by(&id) {
            boundary.push(Value::string(tid_string(model.tid(r))));
        }
    }
    Ok(map(vec![
        ("steepFaces", Value::array(steep)),
        ("nonSteepFaces", Value::array(not_steep)),
        ("boundaryEdges", Value::array(boundary)),
    ]))
}
