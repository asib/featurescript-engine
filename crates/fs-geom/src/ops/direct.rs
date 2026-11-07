//! Direct edits of solids: `@opShell`, `@opDraft`, `@opThicken`, `@opOffsetFace` and `@opDeleteFace`.

use super::{by_body, replace_bodies};
use crate::model::{BodyType, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, field};
use crate::{occt_error, query, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Shape, ShapeKind};

pub fn shell(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let thickness = values::length(field(definition, "thickness"), "opShell thickness")?;
    let entities = field(definition, "entities");
    let mut results = Vec::new();
    // Faces to remove, by body; bodies selected whole are hollowed.
    let faces = by_body(model, entities, EntityType::Face)?;
    for (b, faces) in &faces {
        let op = occt::shell(&model.bodies[*b].shape, faces, thickness).map_err(|_| regen_error("SHELL_FAILED"))?;
        results.push((*b, op));
    }
    let hollow: Vec<usize> = query::evaluate(model, entities)?
        .into_iter()
        .filter(|r| r.kind == EntityType::Body && !faces.iter().any(|(b, _)| *b == r.body))
        .map(|r| r.body)
        .collect();
    for b in hollow {
        let solid = model.bodies[b].shape.clone();
        // A hollow body: the solid less its inward offset (or, outward, its outward offset less the solid).
        let offset = occt::offset(&solid, thickness).map_err(|_| regen_error("SHELL_FAILED"))?;
        let inner = occt::solid_from(&offset).map_err(|_| regen_error("SHELL_FAILED"))?;
        let op = if thickness < 0.0 {
            occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(&solid), &[inner], false)
        } else {
            occt::boolean(occt::BooleanKind::Subtraction, &[inner], std::slice::from_ref(&solid), false)
        }
        .map_err(|_| regen_error("SHELL_FAILED"))?;
        results.push((b, op));
    }
    if results.is_empty() {
        return Err(regen_error("SHELL_SELECT_FACES"));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

pub fn draft(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    match field(definition, "draftType").as_str() {
        None | Some("REFERENCE_SURFACE") => {}
        Some(other) => return Err(error(format!("@opDraft: {other} drafts aren't supported yet"))),
    }
    let pull = values::normalize(values::vec3(field(definition, "pullVec"), "opDraft pullVec")?)
        .ok_or_else(|| error("@opDraft: zero pull direction"))?;
    let angle = values::angle(field(definition, "angle"), "opDraft angle")?;
    let reference = field(definition, "referenceSurface");
    let neutral = if reference.get("normal").is_some() {
        values::plane(reference, "opDraft referenceSurface")?
    } else {
        let found = query::evaluate(model, reference)?;
        let face =
            found.iter().find(|r| r.kind == EntityType::Face).ok_or_else(|| regen_error("DRAFT_NO_NEUTRAL_PLANE"))?;
        model.shape(*face).face_plane().map_err(|_| regen_error("DRAFT_NO_NEUTRAL_PLANE"))?
    };
    let groups = by_body(model, field(definition, "draftFaces"), EntityType::Face)?;
    if groups.is_empty() {
        return Err(regen_error("DRAFT_SELECT_FACES"));
    }
    let mut results = Vec::new();
    for (b, faces) in groups {
        let op = occt::draft(&model.bodies[b].shape, &faces, pull, angle, neutral)
            .map_err(|_| regen_error("DRAFT_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

pub fn thicken(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let t1 = values::length(field(definition, "thickness1"), "opThicken thickness1")?;
    let t2 = values::length(field(definition, "thickness2"), "opThicken thickness2")?;
    let keep_tools = values::opt_bool(definition, "keepTools")?.unwrap_or(true);
    let found = query::evaluate(model, field(definition, "entities"))?;
    if found.is_empty() {
        return Err(regen_error("THICKEN_SELECT_ENTITIES"));
    }
    // Faces (of solids or sheets) and whole sheet bodies, each thickened on its own.
    let mut inputs: Vec<Shape> = Vec::new();
    let mut whole_sheets = Vec::new();
    for r in &found {
        match r.kind {
            EntityType::Face => inputs.push(model.shape(*r).clone()),
            EntityType::Body if model.body(*r).body_type == BodyType::Sheet => {
                inputs.push(model.body(*r).shape.clone());
                whole_sheets.push(r.body);
            }
            _ => {}
        }
    }
    let info = Info::new(id);
    let mut made = Vec::new();
    for input in &inputs {
        let op = occt::thicken(input, t1, t2).map_err(|_| regen_error("THICKEN_FAILED"))?;
        for solid in op.shape().sub_shapes(ShapeKind::Solid) {
            made.push(solid);
        }
    }
    if !keep_tools {
        model.remove_bodies(&whole_sheets);
    }
    for solid in made {
        model.add_body(NewBody::new(solid, BodyType::Solid, info.clone()), &Origins::default(), &info, &|_| None);
    }
    Ok(Value::UNDEFINED)
}

pub fn offset_face(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let distance = values::length(field(definition, "offsetDistance"), "opOffsetFace offsetDistance")?;
    let groups = by_body(model, field(definition, "moveFaces"), EntityType::Face)?;
    if groups.is_empty() {
        return Err(regen_error("DIRECT_EDIT_OFFSET_FACE_SELECT"));
    }
    let mut results = Vec::new();
    for (b, faces) in groups {
        let op = occt::offset_faces(&model.bodies[b].shape, &faces, distance)
            .map_err(|_| regen_error("DIRECT_EDIT_OFFSET_FACE_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

pub fn delete_face(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let leave_open = values::opt_bool(definition, "leaveOpen")?.unwrap_or(false);
    let groups = by_body(model, field(definition, "deleteFaces"), EntityType::Face)?;
    if groups.is_empty() {
        return Err(regen_error("DIRECT_EDIT_DELETE_SELECT_FACES"));
    }
    if leave_open {
        // The body becomes a sheet of its other faces; they keep their identities.
        let info = Info::new(id);
        let mut added = Vec::new();
        for (b, faces) in &groups {
            let old = model.bodies[*b].clone();
            let sheet = occt::remove_faces_open(&old.shape, faces).map_err(occt_error)?;
            let mut origins = Origins::default();
            for entity in old.all_entities() {
                origins.keep(entity);
            }
            added.push((NewBody { body_type: BodyType::Sheet, ..NewBody::replacing(&old, sheet) }, origins));
        }
        let removed: Vec<usize> = groups.iter().map(|(b, _)| *b).collect();
        model.remove_bodies(&removed);
        for (new, origins) in added {
            model.add_body(new, &origins, &info, &|_| None);
        }
        return Ok(Value::UNDEFINED);
    }
    // Capping replaces a face with a planar one, which only works when the face's boundary is flat; Onshape leaves
    // other faces (a hole through a block, say) as they are. (Batch 003, c039.)
    let cap = values::opt_bool(definition, "capVoid")?.unwrap_or(false);
    let mut results = Vec::new();
    for (b, faces) in groups {
        let faces: Vec<Shape> = if cap { faces.into_iter().filter(boundary_is_flat).collect() } else { faces };
        if faces.is_empty() {
            continue;
        }
        let op = occt::delete_faces(&model.bodies[b].shape, &faces)
            .map_err(|_| regen_error("DIRECT_EDIT_DELETE_FACE_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

/// Whether a face's boundary lies in one plane.
fn boundary_is_flat(face: &Shape) -> bool {
    let points: Vec<occt::Vec3> =
        face.sub_shapes(ShapeKind::Vertex).iter().filter_map(|v| v.vertex_point().ok()).collect();
    let edges = face.sub_shapes(ShapeKind::Edge);
    let mut samples = points;
    for edge in &edges {
        for t in [0.25, 0.5, 0.75] {
            if let Ok(axis) = edge.edge_tangent(t, false) {
                samples.push(axis.origin);
            }
        }
    }
    if samples.len() < 3 {
        return true;
    }
    let origin = samples[0];
    let mut normal = None;
    for i in 1..samples.len() {
        for j in i + 1..samples.len() {
            let n = values::cross(values::sub(samples[i], origin), values::sub(samples[j], origin));
            if values::norm(n) > 1e-12 {
                normal = values::normalize(n);
                break;
            }
        }
        if normal.is_some() {
            break;
        }
    }
    let Some(normal) = normal else { return true };
    samples.iter().all(|p| values::dot(values::sub(*p, origin), normal).abs() < 1e-7)
}

/// The plane of the first planar face a query finds.
fn plane_of(model: &Model, query: &Value) -> R<Option<occt::Frame>> {
    let found = query::evaluate(model, query)?;
    Ok(found.iter().filter(|r| r.kind == EntityType::Face).find_map(|r| model.shape(*r).face_plane().ok()))
}

pub fn replace_face(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let mut target = plane_of(model, field(definition, "templateFace"))?
        .ok_or_else(|| error("@opReplaceFace: only planar template faces are supported yet"))?;
    let offset = field(definition, "offset");
    if !offset.is_undefined() {
        let offset = values::length(offset, "opReplaceFace offset")?;
        target.origin = values::add(target.origin, values::scale(target.normal, offset));
    }
    let groups = by_body(model, field(definition, "replaceFaces"), EntityType::Face)?;
    if groups.is_empty() {
        return Err(regen_error("DIRECT_EDIT_REPLACE_FACE_SELECT"));
    }
    let mut results = Vec::new();
    for (b, faces) in groups {
        let mut shape = model.bodies[b].shape.clone();
        let mut last = None;
        for face in &faces {
            let op = occt::replace_planar_face(&shape, face, target)
                .map_err(|_| regen_error("DIRECT_EDIT_REPLACE_FACE_FAILED"))?;
            shape = op.shape();
            last = Some(op);
        }
        if let Some(op) = last {
            results.push((b, op));
        }
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

pub fn move_face(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let t = super::transform(field(definition, "transform"))?;
    let groups = by_body(model, field(definition, "moveFaces"), EntityType::Face)?;
    if groups.is_empty() {
        return Err(regen_error("DIRECT_EDIT_MOVE_FACE_SELECT"));
    }
    let linear_is_identity =
        (0..3).all(|i| (0..3).all(|j| (t.m[i * 4 + j] - f64::from(u8::from(i == j))).abs() < 1e-12));
    let translation = occt::Vec3::new(t.m[3], t.m[7], t.m[11]);
    let failed = || regen_error("DIRECT_EDIT_MOVE_FACE_FAILED");
    let mut results = Vec::new();
    for (b, faces) in groups {
        let solid = model.bodies[b].shape.clone();
        let planes: Vec<Option<occt::Frame>> = faces.iter().map(|f| f.face_plane().ok()).collect();
        // A planar face moved along its own normal is an offset.
        if linear_is_identity
            && faces.len() == 1
            && let Some(plane) = planes[0]
        {
            let along = values::dot(translation, plane.normal);
            if values::norm(values::sub(translation, values::scale(plane.normal, along))) < 1e-12 {
                results.push((b, occt::offset_faces(&solid, &faces, along).map_err(|_| failed())?));
                continue;
            }
        }
        // One planar face turned or moved: replace it with its moved plane.
        if faces.len() == 1
            && let Some(plane) = planes[0]
        {
            let moved = occt::transform(&faces[0], &t).map_err(occt_error)?.shape();
            let target = moved.face_plane().map_err(|_| failed())?;
            debug_assert!(values::norm(plane.normal) > 0.0);
            if let Ok(op) = occt::replace_planar_face(&solid, &faces[0], target) {
                results.push((b, op));
                continue;
            }
        }
        // A hole or boss: heal it away, then cut or add its region moved.
        let healed = occt::delete_faces(&solid, &faces).map_err(|_| failed())?.shape();
        let (healed_volume, volume) = (healed.volume().map_err(occt_error)?, solid.volume().map_err(occt_error)?);
        let subtract = |a: &Shape, b: &Shape| {
            occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(a), std::slice::from_ref(b), true)
        };
        let op = if healed_volume > volume {
            let region = subtract(&healed, &solid).map_err(|_| failed())?.shape();
            let moved = occt::transform(&region, &t).map_err(occt_error)?.shape();
            subtract(&healed, &moved).map_err(|_| failed())?
        } else {
            let region = subtract(&solid, &healed).map_err(|_| failed())?.shape();
            let moved = occt::transform(&region, &t).map_err(occt_error)?.shape();
            occt::boolean(occt::BooleanKind::Union, &[healed], &[moved], true).map_err(|_| failed())?
        };
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}
