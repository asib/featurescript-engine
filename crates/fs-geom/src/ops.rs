//! Operations (`@op*`): the builtins that change a context's geometry.

use crate::model::{BodyType, Cap, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, field, scale};
use crate::{GeomBuiltins, model_mut, occt_error, query, regen_error};
use fs_interp::Value;
use fs_interp::builtins::{arg, context_arg};
use fs_interp::eval::{Flow, R, error};
use occt::{BooleanKind, Shape, ShapeKind, Transform, Vec3};
use std::collections::HashMap;
use std::rc::Rc;

pub fn register(m: &mut GeomBuiltins) {
    m.insert("opPoint", |_, args| {
        run_op(&args, "opPoint", |model, id, definition| {
            let point = values::vec3(field(definition, "point"), "opPoint point")?;
            let construction = values::opt_bool(definition, "origin")?.unwrap_or(false);
            let shape = occt::vertex(point).map_err(occt_error)?;
            add_simple(model, shape, BodyType::Point, &Info::new(id), construction);
            Ok(Value::UNDEFINED)
        })
    });
    m.insert("opPlane", |_, args| {
        run_op(&args, "opPlane", |model, id, definition| {
            let plane = values::plane(field(definition, "plane"), "opPlane plane")?;
            let size = |key: &str| -> R<f64> {
                match field(definition, key) {
                    v if v.is_undefined() => Ok(0.15),
                    v => values::length(v, key),
                }
            };
            let (width, height) = (size("width")?, size("height")?);
            let shape =
                occt::plane_face(plane, -width / 2.0, width / 2.0, -height / 2.0, height / 2.0).map_err(occt_error)?;
            add_simple(model, shape, BodyType::Sheet, &Info::new(id), true);
            Ok(Value::UNDEFINED)
        })
    });
    m.insert("opExtrude", |_, args| run_op(&args, "opExtrude", extrude));
    m.insert("opBoolean", |_, args| run_op(&args, "opBoolean", boolean));
    m.insert("opDeleteBodies", |_, args| {
        run_op(&args, "opDeleteBodies", |model, _, definition| {
            let bodies = owner_bodies(model, field(definition, "entities"))?;
            model.remove_bodies(&bodies);
            Ok(Value::UNDEFINED)
        })
    });
    m.insert("opTransform", |_, args| run_op(&args, "opTransform", transform_bodies));
}

/// Runs an operation on the model. Like Onshape's, a failing operation records its error as the status of the id
/// it was given before throwing: std sometimes calls operations inside `try` and then reads that status.
fn run_op(args: &[Value], name: &str, f: impl FnOnce(&mut Model, FeatureId, &Value) -> R<Value>) -> R<Value> {
    let (id, definition) = id_and_definition(args, name)?;
    let result = model_mut(args, name, |model| f(model, id, &definition));
    if let Err(Flow::Throw(error)) = &result {
        let status = error.get("message").and_then(Value::as_str).unwrap_or("REGEN_ERROR");
        context_arg(args, 0, name)?.report_status(&args[1], "ERROR", status);
    }
    result
}

fn transform_bodies(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let transform = transform(field(definition, "transform"))?;
    let bodies = owner_bodies(model, field(definition, "bodies"))?;
    let mut replacements = Vec::new();
    for &b in &bodies {
        let body = model.bodies[b].clone();
        let op = occt::transform(&body.shape, &transform).map_err(occt_error)?;
        let origins = Origins::of_operation(&op, &[&body]);
        replacements.push((op.shape(), body, origins));
    }
    model.remove_bodies(&bodies);
    let default = Info::new(id);
    for (shape, body, origins) in replacements {
        model.add_body(NewBody::replacing(&body, shape), &origins, &default, &|_| None);
    }
    Ok(Value::UNDEFINED)
}

fn id_and_definition(args: &[Value], name: &str) -> R<(FeatureId, Value)> {
    let id = values::feature_id(arg(args, 1, name)?, &format!("@{name} id"))?;
    Ok((id, arg(args, 2, name)?.clone()))
}

fn add_simple(model: &mut Model, shape: Shape, body_type: BodyType, info: &Rc<Info>, construction: bool) -> usize {
    let new = NewBody { construction, ..NewBody::new(shape, body_type, info.clone()) };
    model.add_body(new, &Origins::default(), info, &|_| None)
}

/// The distinct bodies that own what a query finds, in model order.
fn owner_bodies(model: &Model, query: &Value) -> R<Vec<usize>> {
    let mut bodies: Vec<usize> = query::evaluate(model, query)?.into_iter().map(|r| r.body).collect();
    bodies.sort_unstable();
    bodies.dedup();
    Ok(bodies)
}

/// A `Transform` map: `linear` (a 3×3 matrix as an array of rows) and `translation` (a length vector).
fn transform(v: &Value) -> R<Transform> {
    let linear = field(v, "linear");
    let rows = linear
        .as_array()
        .filter(|rows| rows.len() == 3)
        .ok_or_else(|| error("transform: linear must be a 3x3 matrix"))?;
    let translation = values::vec3(field(v, "translation"), "transform translation")?;
    let mut m = [0.0; 12];
    for (i, row) in rows.iter().enumerate() {
        let row = row.as_array().filter(|r| r.len() == 3).ok_or_else(|| error("transform: bad matrix row"))?;
        for (j, x) in row.iter().enumerate() {
            m[i * 4 + j] = x.as_number().ok_or_else(|| error("transform: matrix entries must be numbers"))?;
        }
    }
    m[3] = translation.x;
    m[7] = translation.y;
    m[11] = translation.z;
    Ok(Transform { m })
}

fn translation(v: Vec3) -> Transform {
    Transform { m: [1.0, 0.0, 0.0, v.x, 0.0, 1.0, 0.0, v.y, 0.0, 0.0, 1.0, v.z] }
}

/// Shapes looked up by [`Shape::is_same`].
#[derive(Default)]
struct ShapeSet(HashMap<u64, Vec<Shape>>);

impl ShapeSet {
    fn insert(&mut self, s: &Shape) {
        let entries = self.0.entry(s.hash()).or_default();
        if !entries.iter().any(|e| e.is_same(s)) {
            entries.push(s.clone());
        }
    }

    /// Adds a shape and its faces, edges and vertices.
    fn insert_all(&mut self, s: &Shape) {
        self.insert(s);
        for kind in [ShapeKind::Face, ShapeKind::Edge, ShapeKind::Vertex] {
            for part in s.sub_shapes(kind) {
                self.insert(&part);
            }
        }
    }

    fn contains(&self, s: &Shape) -> bool {
        self.0.get(&s.hash()).is_some_and(|entries| entries.iter().any(|e| e.is_same(s)))
    }
}

fn extrude(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let found = query::evaluate(model, field(definition, "entities"))?;
    let faces: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Face).map(|r| model.shape(*r).clone()).collect();
    if faces.is_empty() {
        if found.iter().any(|r| r.kind == EntityType::Edge) {
            return Err(error("@opExtrude: extruding edges into sheets isn't supported yet"));
        }
        return Err(regen_error("EXTRUDE_NO_SELECTED_REGION"));
    }
    let direction = values::normalize(values::vec3(field(definition, "direction"), "opExtrude direction")?)
        .ok_or_else(|| error("@opExtrude: zero direction"))?;
    let depth = |bound: &str, depth: &str| -> R<Option<f64>> {
        match field(definition, bound).as_str() {
            None if bound == "startBound" => Ok(None),
            Some("BLIND") => values::length(field(definition, depth), depth).map(Some),
            other => Err(error(format!("@opExtrude: {bound} {other:?} isn't supported yet"))),
        }
    };
    // Positions along `direction`: the end is at `endDepth`; the start is `startDepth` back from the profile, or
    // forward when `isStartBoundOpposite` is false (std's symmetric extrude passes a negative depth that way).
    let end = depth("endBound", "endDepth")?.unwrap_or(0.0);
    let start_depth = depth("startBound", "startDepth")?.unwrap_or(0.0);
    let start =
        if values::opt_bool(definition, "isStartBoundOpposite")?.unwrap_or(true) { -start_depth } else { start_depth };
    let length = end - start;
    if length <= 1e-8 {
        return Err(regen_error("EXTRUDE_FAILED"));
    }

    let profile =
        if faces.len() == 1 { faces[0].clone() } else { occt::merge_faces(&faces).map_err(occt_error)?.shape() };
    let profile = if start == 0.0 {
        profile
    } else {
        occt::transform(&profile, &translation(scale(direction, start))).map_err(occt_error)?.shape()
    };
    let op = occt::prism(&profile, scale(direction, length)).map_err(|_| regen_error("EXTRUDE_FAILED"))?;
    let (mut start_cap, mut end_cap) = (ShapeSet::default(), ShapeSet::default());
    for face in op.first() {
        start_cap.insert_all(&face);
    }
    for face in op.last() {
        end_cap.insert_all(&face);
    }
    let created = Info::new(id.clone());
    let start_info = Rc::new(Info { created_by: id.clone(), sketch_entity: None, cap: Some(Cap::Start) });
    let end_info = Rc::new(Info { created_by: id.clone(), sketch_entity: None, cap: Some(Cap::End) });
    let special = |s: &Shape| {
        if end_cap.contains(s) {
            Some(end_info.clone())
        } else if start_cap.contains(s) {
            Some(start_info.clone())
        } else {
            None
        }
    };
    for solid in op.shape().sub_shapes(ShapeKind::Solid) {
        model.add_body(NewBody::new(solid, BodyType::Solid, created.clone()), &Origins::default(), &created, &special);
    }
    Ok(Value::UNDEFINED)
}

fn boolean(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    // Union and intersection combine all the bodies; the subtractions work on each target separately.
    let (kind, per_target) = match field(definition, "operationType").as_str() {
        Some("UNION") => (BooleanKind::Union, false),
        Some("INTERSECTION") => (BooleanKind::Intersection, false),
        Some("SUBTRACTION") => (BooleanKind::Subtraction, true),
        // Removing the complement of the tools from a target keeps the part of the target inside the tools.
        Some("SUBTRACT_COMPLEMENT") => (BooleanKind::Intersection, true),
        other => return Err(error(format!("@opBoolean: operationType {other:?} isn't supported yet"))),
    };
    let tools = owner_bodies(model, field(definition, "tools"))?;
    let targets = match field(definition, "targets") {
        v if v.is_undefined() => Vec::new(),
        v => owner_bodies(model, v)?,
    };
    let keep_tools = values::opt_bool(definition, "keepTools")?.unwrap_or(false);
    let default = Info::new(id);
    let shapes = |bodies: &[usize]| -> Vec<Shape> { bodies.iter().map(|&b| model.bodies[b].shape.clone()).collect() };

    // Each group is (inputs whose identity the results can take, objects, tools); its inputs are replaced.
    let mut groups: Vec<(Vec<usize>, Vec<usize>, Vec<usize>)> = Vec::new();
    let mut consumed: Vec<usize> = Vec::new();
    if per_target {
        if targets.is_empty() {
            return Err(regen_error("BOOLEAN_NEED_ONE_SOLID"));
        }
        let tools: Vec<usize> = tools.into_iter().filter(|t| !targets.contains(t)).collect();
        for &target in &targets {
            groups.push((vec![target], vec![target], tools.clone()));
            consumed.push(target);
        }
        if !keep_tools {
            consumed.extend(&tools);
        }
    } else {
        let mut inputs = targets.clone();
        inputs.extend(tools.iter().filter(|t| !targets.contains(t)));
        if inputs.len() < 2 {
            return Ok(Value::UNDEFINED);
        }
        groups.push((inputs.clone(), vec![inputs[0]], inputs[1..].to_vec()));
        consumed.extend(&inputs);
    }

    let mut results = Vec::new();
    for (inputs, objects, group_tools) in &groups {
        let op = occt::boolean(kind, &shapes(objects), &shapes(group_tools), true)
            .map_err(|e| error(format!("@opBoolean: {e}")))?;
        let input_bodies: Vec<_> = inputs.iter().map(|&b| model.bodies[b].clone()).collect();
        // Faces a tool leaves in the result, such as a pocket's walls, keep the tool's creator.
        let mut history_bodies: Vec<&crate::model::Body> = input_bodies.iter().map(|b| &**b).collect();
        history_bodies.extend(group_tools.iter().filter(|t| !inputs.contains(t)).map(|&t| &*model.bodies[t]));
        let origins = Rc::new(Origins::of_operation(&op, &history_bodies));
        let mut claimed = Vec::new();
        for solid in op.shape().sub_shapes(ShapeKind::Solid) {
            // The result takes the identity of the first input with a face in it.
            let faces = solid.sub_shapes(ShapeKind::Face);
            let owner = input_bodies.iter().position(|body| {
                body.faces.iter().any(|f| {
                    faces.iter().any(|g| g.is_same(&f.shape))
                        || op.modified(&f.shape).iter().any(|m| faces.iter().any(|g| g.is_same(m)))
                })
            });
            let owner = owner.unwrap_or(0);
            let body = &input_bodies[owner];
            let tid = (!claimed.contains(&owner)).then_some(body.tid);
            claimed.push(owner);
            results.push((solid, body.clone(), tid, origins.clone()));
        }
    }
    model.remove_bodies(&consumed);
    for (solid, body, tid, origins) in results {
        let new = NewBody { body_type: BodyType::Solid, tid, ..NewBody::replacing(&body, solid) };
        model.add_body(new, &origins, &default, &|_| None);
    }
    Ok(Value::UNDEFINED)
}
