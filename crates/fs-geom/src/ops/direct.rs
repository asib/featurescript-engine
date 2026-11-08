//! Direct edits of solids: `@opShell`, `@opDraft`, `@opThicken`, `@opOffsetFace` and `@opDeleteFace`.

use std::rc::Rc;

use super::{ShapeSet, by_body, replace_bodies};
use crate::model::{BodyType, Cap, Entity, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, field};
use crate::{occt_error, query, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Shape, ShapeKind, Vec3};

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
    // NEUTRAL_PLANE is the older form of a reference surface draft, with the face in `neutralPlane`.
    let pull = values::normalize(values::vec3(field(definition, "pullVec"), "opDraft pullVec")?)
        .ok_or_else(|| error("@opDraft: zero pull direction"))?;
    let angle = values::angle(field(definition, "angle"), "opDraft angle")?;
    let reference_key = match field(definition, "draftType").as_str() {
        None | Some("REFERENCE_SURFACE") => "referenceSurface",
        Some("NEUTRAL_PLANE") => "neutralPlane",
        Some("REFERENCE_ENTITY") => return draft_about_entities(model, id, definition, pull, angle),
        Some(other) => return Err(error(format!("@opDraft: {other} drafts aren't supported yet"))),
    };
    let reference = field(definition, reference_key);
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

/// A draft of faces each about its own reference edges: each face turns about the plane through its first
/// reference edge square to the pull direction.
fn draft_about_entities(model: &mut Model, id: FeatureId, definition: &Value, pull: Vec3, angle: f64) -> R<Value> {
    let mut groups: Vec<(usize, Vec<Shape>, Vec<occt::Frame>)> = Vec::new();
    for option in field(definition, "referenceEntityDraftOptions").as_array().map(|a| a.to_vec()).unwrap_or_default() {
        let faces = query::evaluate(model, field(&option, "face"))?;
        let Some(face) = faces.into_iter().find(|r| r.kind == EntityType::Face) else { continue };
        let references = query::evaluate(model, field(&option, "references"))?;
        let edge = references.iter().find(|r| r.kind == EntityType::Edge).ok_or_else(|| regen_error("DRAFT_FAILED"))?;
        let at = model.shape(*edge).edge_tangent(0.5, false).map_err(|_| regen_error("DRAFT_FAILED"))?.origin;
        let neutral = occt::Frame { origin: at, normal: pull, x: values::canonical_x(pull) };
        let shape = model.shape(face).clone();
        match groups.iter_mut().find(|(b, _, _)| *b == face.body) {
            Some((_, shapes, frames)) => {
                shapes.push(shape);
                frames.push(neutral);
            }
            None => groups.push((face.body, vec![shape], vec![neutral])),
        }
    }
    if groups.is_empty() {
        return Err(regen_error("DRAFT_SELECT_FACES"));
    }
    let mut results = Vec::new();
    for (b, faces, neutrals) in groups {
        let op = occt::draft_each(&model.bodies[b].shape, &faces, pull, angle, &neutrals)
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
    // Faces (of solids or sheets) and whole sheet bodies, each thickened on its own. What each makes is tracked back
    // to the entities of the body it came from.
    // Faces of one body are thickened together: all of a sheet's faces as the sheet itself.
    let mut inputs: Vec<(Shape, usize)> = Vec::new();
    let mut whole_sheets = Vec::new();
    let mut faces_by_body: Vec<(usize, Vec<Shape>)> = Vec::new();
    for r in &found {
        match r.kind {
            EntityType::Face => match faces_by_body.iter_mut().find(|(b, _)| *b == r.body) {
                Some((_, faces)) => faces.push(model.shape(*r).clone()),
                None => faces_by_body.push((r.body, vec![model.shape(*r).clone()])),
            },
            EntityType::Body if model.body(*r).body_type == BodyType::Sheet => {
                inputs.push((model.body(*r).shape.clone(), r.body));
                whole_sheets.push(r.body);
            }
            _ => {}
        }
    }
    for (b, faces) in faces_by_body {
        if whole_sheets.contains(&b) {
            continue;
        }
        let body = &model.bodies[b];
        let input = if body.body_type == BodyType::Sheet && faces.len() == body.faces.len() {
            body.shape.clone()
        } else if faces.len() == 1 {
            faces[0].clone()
        } else {
            occt::compound(&faces)
        };
        inputs.push((input, b));
    }
    let info = Info::new(id.clone());
    let start_info = Rc::new(Info {
        created_by: id.clone(),
        sketch_entity: None,
        cap: Some(Cap::Start),
        dependency: None,
        ..Default::default()
    });
    let end_info = Rc::new(Info {
        created_by: id,
        sketch_entity: None,
        cap: Some(Cap::End),
        dependency: None,
        ..Default::default()
    });
    let (mut start_cap, mut end_cap) = (ShapeSet::default(), ShapeSet::default());
    let mut made = Vec::new();
    for (input, body) in &inputs {
        let op = occt::thicken(input, t1, t2).map_err(|_| regen_error("THICKEN_FAILED"))?;
        let entities: Vec<&Entity> = model.bodies[*body].all_entities().collect();
        let origins = Rc::new(Origins::derived(&op, &entities));
        for cap in op.first() {
            start_cap.insert_all(&cap);
        }
        for cap in op.last() {
            end_cap.insert_all(&cap);
        }
        for solid in op.shape().sub_shapes(ShapeKind::Solid) {
            made.push((solid, origins.clone()));
        }
    }
    if !keep_tools {
        model.remove_bodies(&whole_sheets);
    }
    let special = |s: &Shape| {
        if end_cap.contains(s) {
            Some(end_info.clone())
        } else if start_cap.contains(s) {
            Some(start_info.clone())
        } else {
            None
        }
    };
    for (solid, origins) in made {
        model.add_body(NewBody::new(solid, BodyType::Solid, info.clone()), &origins, &info, &special);
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
    let mut groups = by_body(model, field(definition, "deleteFaces"), EntityType::Face)?;
    if groups.is_empty() {
        return Err(regen_error("DIRECT_EDIT_DELETE_SELECT_FACES"));
    }
    // Faces of sheets just go: a sheet with none left goes too, the rest keep their identities.
    let (sheets, solids): (Vec<_>, Vec<_>) =
        groups.drain(..).partition(|(b, _)| model.bodies[*b].body_type == BodyType::Sheet);
    groups = solids;
    let mut emptied = Vec::new();
    let mut trimmed = Vec::new();
    for (b, faces) in sheets {
        let old = model.bodies[b].clone();
        if old.faces.iter().all(|f| faces.iter().any(|g| g.is_same(&f.shape))) {
            emptied.push(b);
            continue;
        }
        let sheet = occt::remove_faces_open(&old.shape, &faces).map_err(occt_error)?;
        let mut origins = Origins::default();
        for entity in old.all_entities() {
            origins.keep(entity);
        }
        trimmed.push((b, NewBody { body_type: BodyType::Sheet, ..NewBody::replacing(&old, sheet) }, origins));
    }
    if !emptied.is_empty() || !trimmed.is_empty() {
        let info = Info::new(id.clone());
        let mut removed = emptied;
        removed.extend(trimmed.iter().map(|(b, _, _)| *b));
        // Indices of the solids still to do shift as bodies go; find them again by transient id.
        let solid_tids: Vec<(crate::model::Tid, Vec<Shape>)> =
            groups.drain(..).map(|(b, faces)| (model.bodies[b].tid, faces)).collect();
        model.remove_bodies(&removed);
        for (_, new, origins) in trimmed {
            model.add_body(new, &origins, &info, &|_| None);
        }
        for (tid, faces) in solid_tids {
            if let Some(b) = model.bodies.iter().position(|body| body.tid == tid) {
                groups.push((b, faces));
            }
        }
        if groups.is_empty() {
            return Ok(Value::UNDEFINED);
        }
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
    // The faces replaced, by body: the face on the target plane takes over what they were.
    let mut replaced: Vec<(crate::model::Tid, Entity)> = Vec::new();
    for (b, faces) in groups {
        let body = &model.bodies[b];
        if let Some(first) = body.faces.iter().find(|f| faces.iter().any(|g| g.is_same(&f.shape))) {
            replaced.push((body.tid, first.clone()));
        }
        let mut shape = body.shape.clone();
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
    // The new face keeps the replaced face's info, attributes and identity, as Onshape's does (frame's mitres rely on
    // the cap faces' attributes surviving).
    for (tid, old) in replaced {
        let Some(b) = model.bodies.iter().position(|body| body.tid == tid) else { continue };
        let on_target = model.bodies[b].faces.iter().position(|f| {
            f.shape.face_plane().is_ok_and(|p| {
                values::norm(values::cross(p.normal, target.normal)) < 1e-9
                    && values::dot(values::sub(p.origin, target.origin), target.normal).abs() < 1e-8
            })
        });
        let Some(index) = on_target else { continue };
        model.update_body(b, |body| {
            let face = &mut body.faces[index];
            face.info = old.info.clone();
            face.attributes = old.attributes.clone();
            let mut identity: Vec<crate::model::Tid> = face.ancestry.identity.to_vec();
            identity.push(old.tid);
            identity.extend(old.ancestry.identity.iter().copied());
            face.ancestry.identity = identity.into();
        });
    }
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

/// `@opOffsetWire`: for thin features, the region between planar edges offset `offset1` to one side and `offset2` to
/// the other (sharp corners), a sheet body per chain of edges.
pub fn offset_wire(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    if values::opt_bool(definition, "makeRegions")? != Some(true) {
        return Err(error("@opOffsetWire: only makeRegions is supported yet"));
    }
    let normal = values::normalize(values::vec3(field(definition, "normal"), "opOffsetWire normal")?)
        .ok_or_else(|| error("@opOffsetWire: zero normal"))?;
    let (mut o1, mut o2) = (
        values::length(field(definition, "offset1"), "opOffsetWire offset1")?,
        values::length(field(definition, "offset2"), "opOffsetWire offset2")?,
    );
    if values::opt_bool(definition, "flip")?.unwrap_or(false) {
        std::mem::swap(&mut o1, &mut o2);
    }
    let found = query::evaluate(model, field(definition, "edges"))?;
    let edges: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Edge).map(|r| model.shape(*r).clone()).collect();
    if edges.is_empty() {
        return Err(regen_error("OFFSET_WIRE_SELECT_WALL_PATH"));
    }
    let info = Info::new(id);
    for wire in occt::connect_edges(&edges).map_err(occt_error)? {
        let region =
            occt::offset_wire_region(&wire, normal, o1, o2).map_err(|_| regen_error("OFFSET_WIRE_GENERIC_ERROR"))?;
        model.add_body(NewBody::new(region, BodyType::Sheet, info.clone()), &Origins::default(), &info, &|_| None);
    }
    Ok(Value::UNDEFINED)
}

/// `@opBodyDraft`: tapers steep faces by adding material between reference edges and the parting object. Each face
/// turns about its reference edge: the given top edges, or for faces (or whole parts), the edge farthest along the
/// pull direction where the face meets a face that isn't steep.
pub fn body_draft(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let pull = values::normalize(values::vec3(field(definition, "pullDirection"), "opBodyDraft pullDirection")?)
        .ok_or_else(|| regen_error("BODY_DRAFT_BAD_PULL_DIRECTION"))?;
    let angle = values::angle(field(definition, "angle"), "opBodyDraft angle")?;
    if values::opt_bool(definition, "bothSides")?.unwrap_or(false) {
        return Err(error("@opBodyDraft: drafts on both sides aren't supported yet"));
    }
    // A face is steep if it is closer to the pull direction than the draft angle.
    let normal = |face: &Shape| face.face_tangent_plane(0.5, 0.5).ok().map(|p| p.normal);
    let steep = |face: &Shape| normal(face).is_some_and(|n| values::dot(n, pull).abs() < angle.sin());
    let height = |edge: &Shape| edge.edge_tangent(0.5, false).ok().map(|a| values::dot(a.origin, pull));
    // (body, face, reference edge)
    let mut pairs: Vec<(usize, Shape, Shape)> = Vec::new();
    // Bodies drafted after splitting at the parting plane.
    let mut split_results: Vec<(usize, occt::Operation)> = Vec::new();
    let selection = field(definition, "selectionType").as_str().unwrap_or("EDGES");
    match selection {
        "EDGES" => {
            for r in query::evaluate(model, field(definition, "topEdges"))? {
                if r.kind != EntityType::Edge {
                    continue;
                }
                let edge = model.shape(r).clone();
                let Some(top) = height(&edge) else { continue };
                for face in &model.bodies[r.body].faces {
                    let below = face.shape.bounding_box(false).is_ok_and(|b| {
                        let centre = values::scale(values::add(b.min, b.max), 0.5);
                        values::dot(centre, pull) < top
                    });
                    let has = face.shape.visible_sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(&edge));
                    if has && below && steep(&face.shape) {
                        pairs.push((r.body, face.shape.clone(), edge.clone()));
                    }
                }
            }
        }
        "FACES" | "PARTS" => {
            let mut faces: Vec<(usize, Shape)> = Vec::new();
            if selection == "FACES" {
                for r in query::evaluate(model, field(definition, "faces"))? {
                    if r.kind == EntityType::Face {
                        faces.push((r.body, model.shape(r).clone()));
                    }
                }
            } else {
                let excluded: Vec<Shape> = match field(definition, "excludeFaces") {
                    v if v.is_undefined() => Vec::new(),
                    v => query::evaluate(model, v)?.into_iter().map(|r| model.shape(r).clone()).collect(),
                };
                for b in super::owner_bodies(model, field(definition, "bodies"))? {
                    for face in &model.bodies[b].faces {
                        if steep(&face.shape) && !excluded.iter().any(|e| e.is_same(&face.shape)) {
                            faces.push((b, face.shape.clone()));
                        }
                    }
                }
            }
            // A face's reference edge: its highest edge shared with a face of the solid that isn't steep.
            let reference = |solid: &Shape, face: &Shape| -> Option<Shape> {
                let others: Vec<Shape> =
                    solid.sub_shapes(ShapeKind::Face).into_iter().filter(|f| !f.is_same(face) && !steep(f)).collect();
                face.visible_sub_shapes(ShapeKind::Edge)
                    .into_iter()
                    .filter(|e| others.iter().any(|o| o.sub_shapes(ShapeKind::Edge).iter().any(|x| x.is_same(e))))
                    .filter_map(|e| height(&e).map(|h| (h, e)))
                    .max_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, e)| e)
            };
            // A body the parting plane cuts is split there; only the piece in front of it is drafted, and the
            // pieces are joined again, leaving a ledge at the parting plane. (Batch 007, c019.)
            let parting = parting_plane(model, definition)?;
            let mut bodies: Vec<usize> = faces.iter().map(|(b, _)| *b).collect();
            bodies.dedup();
            for b in bodies {
                let body_faces: Vec<Shape> = faces.iter().filter(|(g, _)| *g == b).map(|(_, f)| f.clone()).collect();
                let shape = model.bodies[b].shape.clone();
                let crossing = parting.filter(|plane| {
                    shape.bounding_box_in(*plane, true).is_ok_and(|bx| bx.min.z < -1e-8 && bx.max.z > 1e-8)
                });
                let Some(plane) = crossing else {
                    for face in body_faces {
                        if let Some(edge) = reference(&shape, &face) {
                            pairs.push((b, face, edge));
                        }
                    }
                    continue;
                };
                let bounds = shape.bounding_box(false).map_err(occt_error)?;
                let size = values::norm(values::sub(bounds.max, bounds.min)) * 4.0 + 1.0;
                let cutter = occt::plane_face(plane, -size, size, -size, size).map_err(occt_error)?;
                let split = occt::split(std::slice::from_ref(&shape), &[cutter])
                    .map_err(|_| regen_error("BODY_DRAFT_FAILED"))?;
                let (mut front, mut back) = (Vec::new(), Vec::new());
                for piece in split.shape().sub_shapes(ShapeKind::Solid) {
                    let in_front = piece.bounding_box_in(plane, true).is_ok_and(|bx| bx.min.z > -1e-8);
                    if in_front { front.push(piece) } else { back.push(piece) }
                }
                let mut drafted = Vec::new();
                for piece in front {
                    let mut piece_faces = Vec::new();
                    let mut neutrals = Vec::new();
                    for face in piece.sub_shapes(ShapeKind::Face) {
                        let selected =
                            face.face_inner_point().ok().and_then(|p| occt::vertex(p).ok()).is_some_and(|p| {
                                body_faces.iter().any(|f| p.distance(f).is_ok_and(|d| d.distance < 1e-8))
                            });
                        if !(selected && steep(&face)) {
                            continue;
                        }
                        let Some(edge) = reference(&piece, &face) else { continue };
                        let at = edge.edge_tangent(0.5, false).map_err(|_| regen_error("BODY_DRAFT_FAILED"))?.origin;
                        neutrals.push(occt::Frame { origin: at, normal: pull, x: values::canonical_x(pull) });
                        piece_faces.push(face);
                    }
                    drafted.push(if piece_faces.is_empty() {
                        piece
                    } else {
                        occt::draft_each(&piece, &piece_faces, pull, angle, &neutrals)
                            .map_err(|_| regen_error("BODY_DRAFT_FAILED"))?
                            .shape()
                    });
                }
                let (first, rest) = drafted.split_first().ok_or_else(|| regen_error("BODY_DRAFT_FAILED"))?;
                let tools: Vec<Shape> = rest.iter().cloned().chain(back).collect();
                let op = occt::boolean(occt::BooleanKind::Union, std::slice::from_ref(first), &tools, true)
                    .map_err(|_| regen_error("BODY_DRAFT_FAILED"))?;
                split_results.push((b, op));
            }
        }
        other => return Err(error(format!("@opBodyDraft: selectionType {other} isn't supported"))),
    }
    let mut groups: Vec<(usize, Vec<Shape>, Vec<occt::Frame>)> = Vec::new();
    for (b, face, edge) in pairs {
        let at = edge.edge_tangent(0.5, false).map_err(|_| regen_error("BODY_DRAFT_FAILED"))?.origin;
        let neutral = occt::Frame { origin: at, normal: pull, x: values::canonical_x(pull) };
        match groups.iter_mut().find(|(g, _, _)| *g == b) {
            Some((_, faces, frames)) => {
                if !faces.iter().any(|f| f.is_same(&face)) {
                    faces.push(face);
                    frames.push(neutral);
                }
            }
            None => groups.push((b, vec![face], vec![neutral])),
        }
    }
    if groups.is_empty() && split_results.is_empty() {
        return Err(regen_error("BODY_DRAFT_FAILED"));
    }
    let mut results = split_results;
    for (b, faces, neutrals) in groups {
        let op = occt::draft_each(&model.bodies[b].shape, &faces, pull, angle, &neutrals)
            .map_err(|_| regen_error("BODY_DRAFT_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

/// The plane of a body draft's parting object, if it is a planar face (or a sheet of one).
fn parting_plane(model: &Model, definition: &Value) -> R<Option<occt::Frame>> {
    let parting = field(definition, "partingObject");
    if parting.is_undefined() {
        return Ok(None);
    }
    if parting.get("normal").is_some() {
        return values::plane(parting, "opBodyDraft partingObject").map(Some);
    }
    for r in query::evaluate(model, parting)? {
        let face = match r.kind {
            EntityType::Face => model.shape(r).clone(),
            EntityType::Body => match model.bodies[r.body].faces.as_slice() {
                [only] => only.shape.clone(),
                _ => continue,
            },
            _ => continue,
        };
        if let Ok(plane) = face.face_plane() {
            return Ok(Some(plane));
        }
    }
    Ok(None)
}
