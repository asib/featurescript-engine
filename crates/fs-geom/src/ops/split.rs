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
            if let Some(frame) = plane {
                let centre = piece.centroid().map_err(occt_error)?;
                let front = values::dot(values::sub(centre, frame.origin), frame.normal) > 0.0;
                if (keep == "KEEP_FRONT" && !front) || (keep == "KEEP_BACK" && front) {
                    continue;
                }
            }
            let tid = first.then_some(body.tid);
            first = false;
            results.push((NewBody { tid, ..NewBody::replacing(&body, piece) }, origins.clone()));
        }
    }
    let mut remove = targets.clone();
    remove.extend(delete_tools);
    model.remove_bodies(&remove);
    for (new, origins) in results {
        model.add_body(new, &origins, &default, &|_| None);
    }
    Ok(Value::UNDEFINED)
}
