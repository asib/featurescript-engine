//! Operations (`@op*`): the builtins that change a context's geometry.

mod blend;
mod curve;
mod direct;
mod extrude;
mod frames;
mod hole;
mod isocline;
mod merge;
mod pattern;
mod revolve;
pub mod sheet_metal;
mod tessellated;
mod split;
mod surface;
mod sweep;

pub use revolve::rotation;

use crate::model::{BodyType, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, field};
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
            // Always a construction point; `origin` only marks the default origin.
            let point = values::vec3(field(definition, "point"), "opPoint point")?;
            let shape = occt::vertex(point).map_err(occt_error)?;
            add_simple(model, shape, BodyType::Point, &Info::new(id), true);
            Ok(Value::UNDEFINED)
        })
    });
    m.insert("opPlane", |_, args| {
        run_op(&args, "opPlane", |model, id, definition| {
            let plane = values::plane(field(definition, "plane"), "opPlane plane")?;
            let plane = occt::Frame { x: values::canonical_x(plane.normal), ..plane };
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
    m.insert("opExtrude", |_, args| run_op(&args, "opExtrude", extrude::extrude));
    m.insert("opBoolean", |_, args| run_op(&args, "opBoolean", boolean));
    m.insert("opDeleteBodies", |_, args| {
        run_op(&args, "opDeleteBodies", |model, _, definition| {
            let bodies = owner_bodies(model, field(definition, "entities"))?;
            // Outside sheet metal features, sheet metal parts can't be deleted: a failed fixture case's sheet metal
            // part stays behind in Onshape (batch 014, c006).
            if !model.sheet_metal_scope
                && bodies
                    .iter()
                    .any(|&b| !model.bodies[b].is_sheet_metal_definition() && sheet_metal::in_active_model(model, b))
            {
                return Err(regen_error("SHEET_METAL_PARTS_PROHIBITED"));
            }
            model.remove_bodies(&bodies);
            Ok(Value::UNDEFINED)
        })
    });
    m.insert("opTransform", |_, args| run_op(&args, "opTransform", transform_bodies));
    m.insert("updateSheetMetalGeometry", |_, args| run_op(&args, "updateSheetMetalGeometry", sheet_metal::update));
    m.insert("opEdgeChange", |_, args| run_op(&args, "opEdgeChange", sheet_metal::edge_change));
    m.insert("sheetMetalApplyInFlat", |_, args| run_op(&args, "sheetMetalApplyInFlat", sheet_metal::apply_in_flat));
    m.insert("evCornerType", |interp, args| {
        let tags = crate::tags(interp)?;
        let definition = fs_interp::builtins::arg(&args, 1, "evCornerType")?.clone();
        model_mut(&args, "evCornerType", |model| sheet_metal::corner_type(&tags, model, &definition))
    });
    // The tools behind sheet metal holes a face pattern carries along (batch 032): each registered tool a hole
    // face lies on, with the wall it's registered on and that wall's frame (the identity: the engine keeps tools
    // where they cut). Forms aren't supported yet.
    m.insert("evSheetMetalHoleToolBodies", |_, args| {
        let definition = fs_interp::builtins::arg(&args, 1, "evSheetMetalHoleToolBodies")?.clone();
        model_mut(&args, "evSheetMetalHoleToolBodies", |model| {
            let found: Vec<crate::model::EntityRef> = query::evaluate(model, field(&definition, "sheetMetalHoleFaces"))?
                .into_iter()
                .filter(|r| r.kind == EntityType::Face)
                .collect();
            let faces: Vec<Shape> = found.iter().map(|r| model.shape(*r).clone()).collect();
            // Only the models the faces' parts belong to.
            let models: Vec<crate::model::Tid> = found
                .iter()
                .filter_map(|r| sheet_metal::definition_of_part(model, r.body))
                .map(|d| model.bodies[d].tid)
                .collect();
            let points: Vec<occt::Vec3> = faces.iter().filter_map(|f| f.face_inner_point().ok()).collect();
            let identity = || {
                let row = |i: usize| Value::array((0..3).map(|j| Value::number(if i == j { 1.0 } else { 0.0 })).collect());
                crate::values::map(vec![
                    ("linear", Value::array((0..3).map(row).collect())),
                    ("translation", Value::array(vec![Value::number(0.0); 3])),
                ])
            };
            let (mut bodies, mut walls, mut transforms) = (Vec::new(), Vec::new(), Vec::new());
            for def in model.bodies.iter().filter(|b| b.is_sheet_metal_definition() && models.contains(&b.tid)) {
                for wall in &def.faces {
                    for tool in sheet_metal::wall_tool_tids_of(&wall.attributes) {
                        let Some(body) = model.bodies.iter().find(|b| b.tid == tool) else { continue };
                        let touches = points.iter().any(|p| {
                            occt::vertex(*p).and_then(|v| v.distance(&body.shape)).is_ok_and(|d| d.distance < 1e-7)
                        });
                        if touches && !bodies.iter().any(|b: &Value| b.as_str() == Some(&query::tid_string(tool))) {
                            bodies.push(Value::string(query::tid_string(tool)));
                            walls.push(Value::string(query::tid_string(wall.tid)));
                            transforms.push(identity());
                        }
                    }
                }
            }
            Ok(crate::values::map(vec![
                ("sheetMetalHoleToolBodies", Value::array(bodies)),
                ("sheetMetalHoleToolWalls", Value::array(walls)),
                ("sheetMetalHoleToolTransforms", Value::array(transforms)),
            ]))
        })
    });
    // The tools behind sheet metal forms a face pattern carries along (batch 033): for each wall of the faces' own
    // model with a form whose part to add or remove a form face lies on, that form's part ids and the wall's frame
    // (the identity: the engine keeps tools where they form).
    m.insert("evSheetMetalFormToolBodies", |interp, args| {
        let tags = crate::tags(interp)?;
        let definition = fs_interp::builtins::arg(&args, 1, "evSheetMetalFormToolBodies")?.clone();
        model_mut(&args, "evSheetMetalFormToolBodies", |model| {
            let found: Vec<crate::model::EntityRef> = query::evaluate(model, field(&definition, "sheetMetalFormFaces"))?
                .into_iter()
                .filter(|r| r.kind == EntityType::Face)
                .collect();
            let points: Vec<occt::Vec3> = found.iter().filter_map(|r| model.shape(*r).face_inner_point().ok()).collect();
            let models: Vec<crate::model::Tid> = found
                .iter()
                .filter_map(|r| sheet_metal::definition_of_part(model, r.body))
                .map(|d| model.bodies[d].tid)
                .collect();
            let identity = tags.transform([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], occt::Vec3::new(0.0, 0.0, 0.0));
            let mut out: Vec<(&str, Value)> = Vec::new();
            let mut keys: Vec<String> = Vec::new();
            let mut values: Vec<Value> = Vec::new();
            for def in model.bodies.iter().filter(|b| b.is_sheet_metal_definition() && models.contains(&b.tid)) {
                for wall in &def.faces {
                    let mut tools = Vec::new();
                    for (positive, negative) in sheet_metal::wall_form_parts(&wall.attributes) {
                        let touches = |t: Option<crate::model::Tid>| {
                            t.and_then(|t| model.bodies.iter().find(|b| b.tid == t)).is_some_and(|body| {
                                points.iter().any(|p| {
                                    occt::vertex(*p).and_then(|v| v.distance(&body.shape)).is_ok_and(|d| d.distance < 1e-7)
                                })
                            })
                        };
                        if touches(positive) || touches(negative) {
                            if let Some(t) = positive.or(negative) {
                                tools.push(Value::string(query::tid_string(t)));
                            }
                        }
                    }
                    if !tools.is_empty() {
                        keys.push(query::tid_string(wall.tid));
                        values.push(crate::values::map(vec![
                            ("formToolBodies", Value::array(tools)),
                            ("transform", identity.clone()),
                        ]));
                    }
                }
            }
            for (k, v) in keys.iter().zip(values) {
                out.push((k.as_str(), v));
            }
            Ok(crate::values::map(out))
        })
    });
    m.insert("opTessellatedLoft", |_, args| run_op(&args, "opTessellatedLoft", tessellated::loft));
    m.insert("evTessellatedLoftMatches", |interp, args| {
        let tags = crate::tags(interp)?;
        let definition = fs_interp::builtins::arg(&args, 1, "evTessellatedLoftMatches")?.clone();
        model_mut(&args, "evTessellatedLoftMatches", |model| tessellated::matches(&tags, model, &definition))
    });
    m.insert("opRevolve", |_, args| run_op(&args, "opRevolve", revolve::revolve));
    m.insert("opSphere", |_, args| {
        run_op(&args, "opSphere", |model, id, definition| {
            let center = values::vec3(field(definition, "center"), "opSphere center")?;
            let radius = values::length(field(definition, "radius"), "opSphere radius")?;
            let shape = occt::sphere(center, radius).map_err(occt_error)?;
            add_simple(model, shape, BodyType::Solid, &Info::new(id), false);
            Ok(Value::UNDEFINED)
        })
    });
    m.insert("opHole", |_, args| run_op(&args, "opHole", hole::hole));
    m.insert("opMergeContexts", |_, args| {
        let (_, definition) = id_and_definition(&args, "opMergeContexts")?;
        let source = crate::model_of(field(&definition, "contextFrom"), "opMergeContexts", |other| {
            merge::read(other, &definition)
        })?;
        run_op(&args, "opMergeContexts", |model, id, definition| merge::merge(model, id, definition, &source))
    });
    m.insert("opCreateIsocline", |_, args| run_op(&args, "opCreateIsocline", isocline::create));
    m.insert("opSplitByIsocline", |_, args| run_op(&args, "opSplitByIsocline", isocline::split));
    m.insert("opNameEntity", |_, args| {
        run_op(&args, "opNameEntity", |model, _, definition| {
            let name: Rc<str> = field(definition, "entityName").as_str().unwrap_or_default().into();
            let found = query::evaluate(model, field(definition, "entity"))?;
            let tids: Vec<crate::model::Tid> = found.iter().map(|r| model.tid(*r)).collect();
            Rc::make_mut(&mut model.names).extend(tids.into_iter().map(|t| (name.clone(), t)));
            Ok(Value::UNDEFINED)
        })
    });
    m.insert("opCreateCompositePart", |_, args| {
        run_op(&args, "opCreateCompositePart", |model, id, definition| {
            // A body holding other bodies: its shape is theirs together, its entities are theirs.
            let bodies = owner_bodies(model, field(definition, "bodies"))?;
            let bodies: Vec<usize> = bodies.into_iter().filter(|&b| model.bodies[b].composite.is_none()).collect();
            if bodies.is_empty() {
                return Err(regen_error("COMPOSITE_PART_SELECT_BODIES"));
            }
            let closed = values::opt_bool(definition, "closed")?.unwrap_or(false);
            let shapes: Vec<Shape> = bodies.iter().map(|&b| model.bodies[b].shape.clone()).collect();
            let members: Rc<[crate::model::Tid]> = bodies.iter().map(|&b| model.bodies[b].tid).collect();
            let info = Info::new(id);
            let mut new = NewBody::new(occt::compound(&shapes), BodyType::Composite, info.clone());
            new.composite = Some((members, closed));
            model.add_body(new, &Origins::default(), &info, &|_| None);
            Ok(Value::UNDEFINED)
        })
    });
    m.insert("opPattern", |_, args| run_op(&args, "opPattern", pattern::pattern));
    m.insert("opSplitPart", |_, args| run_op(&args, "opSplitPart", split::split_part));
    m.insert("opSplitFace", |interp, args| {
        let tags = crate::tags(interp)?;
        run_op(&args, "opSplitFace", |model, id, definition| split::split_face(model, id, definition, &tags))
    });
    m.insert("opFillet", |_, args| run_op(&args, "opFillet", blend::fillet));
    m.insert("opChamfer", |_, args| run_op(&args, "opChamfer", blend::chamfer));
    m.insert("opModifyFillet", |_, args| run_op(&args, "opModifyFillet", blend::modify_fillet));
    m.insert("opFullRoundFillet", |_, args| run_op(&args, "opFullRoundFillet", blend::full_round));
    m.insert("opFaceBlend", |_, args| run_op(&args, "opFaceBlend", blend::face_blend));
    m.insert("opShell", |_, args| run_op(&args, "opShell", direct::shell));
    m.insert("opDraft", |_, args| run_op(&args, "opDraft", direct::draft));
    m.insert("opBodyDraft", |_, args| run_op(&args, "opBodyDraft", direct::body_draft));
    m.insert("opThicken", |_, args| run_op(&args, "opThicken", direct::thicken));
    m.insert("opOffsetFace", |_, args| run_op(&args, "opOffsetFace", direct::offset_face));
    m.insert("opDeleteFace", |_, args| run_op(&args, "opDeleteFace", direct::delete_face));
    m.insert("opReplaceFace", |_, args| run_op(&args, "opReplaceFace", direct::replace_face));
    m.insert("opMoveFace", |_, args| run_op(&args, "opMoveFace", direct::move_face));
    m.insert("opOffsetWire", |_, args| run_op(&args, "opOffsetWire", direct::offset_wire));
    m.insert("opSweep", |_, args| run_op(&args, "opSweep", sweep::sweep));
    m.insert("opLoft", |_, args| run_op(&args, "opLoft", sweep::loft));
    m.insert("opHelix", |_, args| run_op(&args, "opHelix", curve::helix));
    m.insert("opFitSpline", |_, args| run_op(&args, "opFitSpline", curve::fit_spline));
    m.insert("opCreateBSplineCurve", |_, args| run_op(&args, "opCreateBSplineCurve", curve::bspline_curve));
    m.insert("opMateConnector", |_, args| run_op(&args, "opMateConnector", crate::mate::make_connector));
    m.insert("opExtractWires", |_, args| run_op(&args, "opExtractWires", surface::extract_wires));
    m.insert("opEditCurve", |_, args| run_op(&args, "opEditCurve", curve::edit_curve));
    m.insert("opPolyline", |_, args| run_op(&args, "opPolyline", curve::polyline));
    m.insert("opConstrainedSurface", |_, args| run_op(&args, "opConstrainedSurface", surface::constrained_surface));
    m.insert("opWrap", |_, args| run_op(&args, "opWrap", surface::wrap));
    m.insert("opSplineThroughEdges", |_, args| run_op(&args, "opSplineThroughEdges", surface::spline_through_edges));
    m.insert("opExtractSurface", |_, args| run_op(&args, "opExtractSurface", surface::extract_surface));
    m.insert("opIntersectFaces", |_, args| run_op(&args, "opIntersectFaces", surface::intersect_faces));
    m.insert("opFillSurface", |_, args| run_op(&args, "opFillSurface", surface::fill_surface));
    m.insert("opExtendSheetBody", |_, args| run_op(&args, "opExtendSheetBody", surface::extend_sheet));
    m.insert("opCreateCurvesOnFace", |_, args| run_op(&args, "opCreateCurvesOnFace", surface::curves_on_face));
    m.insert("opBoundarySurface", |_, args| run_op(&args, "opBoundarySurface", surface::boundary_surface));
    m.insert("opOffsetCurveOnFace", |_, args| run_op(&args, "opOffsetCurveOnFace", surface::offset_curve_on_face));
    m.insert("opEnclose", |_, args| run_op(&args, "opEnclose", surface::enclose));
    m.insert("opDropCurve", |_, args| run_op(&args, "opDropCurve", surface::drop_curve));
    m.insert("opRuledSurface", |_, args| run_op(&args, "opRuledSurface", surface::ruled_surface));
    m.insert("opMoveCurveBoundary", |_, args| run_op(&args, "opMoveCurveBoundary", surface::move_curve_boundary));
}

/// The shapes of the entities a query finds, grouped by the body that owns them, in model order.
pub(crate) fn by_body(model: &Model, query: &Value, kind: EntityType) -> R<Vec<(usize, Vec<Shape>)>> {
    let mut groups: Vec<(usize, Vec<Shape>)> = Vec::new();
    for r in query::evaluate(model, query)?.into_iter().filter(|r| r.kind == kind) {
        let shape = model.shape(r).clone();
        match groups.iter_mut().find(|(b, _)| *b == r.body) {
            Some((_, shapes)) => shapes.push(shape),
            None => groups.push((r.body, vec![shape])),
        }
    }
    groups.sort_by_key(|(b, _)| *b);
    Ok(groups)
}

/// Replaces bodies with the results of operations on them: each body's pieces keep its identity (the first piece
/// its transient id), and entities follow the operation's history; new ones are created by `id`.
pub(crate) fn replace_bodies(model: &mut Model, id: &FeatureId, results: Vec<(usize, occt::Operation)>) {
    let default = Info::new(id.clone());
    let mut added = Vec::new();
    for (b, op) in &results {
        let old = model.bodies[*b].clone();
        let origins = Rc::new(Origins::of_operation(op, &[&old]));
        let shape = op.shape();
        let solids = shape.sub_shapes(ShapeKind::Solid);
        let pieces = if solids.is_empty() { vec![shape] } else { solids };
        for (i, piece) in pieces.into_iter().enumerate() {
            let body_type = BodyType::of(&piece);
            let tid = (i == 0).then_some(old.tid);
            added.push((NewBody { tid, body_type, ..NewBody::replacing(&old, piece) }, origins.clone()));
        }
    }
    let removed: Vec<usize> = results.iter().map(|(b, _)| *b).collect();
    model.remove_bodies(&removed);
    for (new, origins) in added {
        model.add_body(new, &origins, &default, &|_| None);
    }
}

/// Runs an operation on the model. Like Onshape's, a failing operation records its error as the status of the id
/// it was given before throwing: std sometimes calls operations inside `try` and then reads that status.
fn run_op(args: &[Value], name: &str, f: impl FnOnce(&mut Model, FeatureId, &Value) -> R<Value>) -> R<Value> {
    let (id, definition) = id_and_definition(args, name)?;
    let result = model_mut(args, name, |model| {
        model.last_operation = Some(id.clone());
        f(model, id, &definition)
    });
    if let Err(Flow::Throw(error)) = &result {
        let status = error.get("message").and_then(Value::as_str).unwrap_or("REGEN_ERROR");
        context_arg(args, 0, name)?.report_status(&args[1], "ERROR", status);
    }
    result
}

fn transform_bodies(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let transform = transform(field(definition, "transform"))?;
    let bodies = owner_bodies(model, field(definition, "bodies"))?;
    // Sheet metal parts move only with their model, which opTransform can't do (batch 012); the features move
    // their definition sheets themselves.
    if bodies.iter().any(|&b| !model.bodies[b].is_sheet_metal_definition() && sheet_metal::in_active_model(model, b)) {
        return Err(regen_error("SHEET_METAL_PARTS_PROHIBITED"));
    }
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
        let frame = body.frame.map(|f| transform_frame(&f, &transform));
        model.add_body(NewBody { frame, ..NewBody::replacing(&body, shape) }, &origins, &default, &|_| None);
    }
    Ok(Value::UNDEFINED)
}

pub(crate) fn id_and_definition(args: &[Value], name: &str) -> R<(FeatureId, Value)> {
    let id = values::feature_id(arg(args, 1, name)?, &format!("@{name} id"))?;
    Ok((id, arg(args, 2, name)?.clone()))
}

pub(crate) fn add_simple(
    model: &mut Model,
    shape: Shape,
    body_type: BodyType,
    info: &Rc<Info>,
    construction: bool,
) -> usize {
    let new = NewBody { construction, ..NewBody::new(shape, body_type, info.clone()) };
    model.add_body(new, &Origins::default(), info, &|_| None)
}

/// The distinct bodies that own what a query finds, in model order.
pub(crate) fn owner_bodies(model: &Model, query: &Value) -> R<Vec<usize>> {
    let mut bodies: Vec<usize> = query::evaluate(model, query)?.into_iter().map(|r| r.body).collect();
    bodies.sort_unstable();
    bodies.dedup();
    Ok(bodies)
}

/// A `Transform` map: `linear` (a 3×3 matrix as an array of rows) and `translation` (a length vector).
pub(crate) fn transform(v: &Value) -> R<Transform> {
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

/// A coordinate system moved by a transform: its origin moved, its axes turned (and scaled back to unit length).
pub(crate) fn transform_frame(f: &occt::Frame, t: &Transform) -> occt::Frame {
    let m = &t.m;
    let linear = |v: Vec3| {
        Vec3::new(
            m[0] * v.x + m[1] * v.y + m[2] * v.z,
            m[4] * v.x + m[5] * v.y + m[6] * v.z,
            m[8] * v.x + m[9] * v.y + m[10] * v.z,
        )
    };
    let origin = values::add(linear(f.origin), Vec3::new(m[3], m[7], m[11]));
    let unit = |v: Vec3| values::normalize(v).unwrap_or(v);
    occt::Frame { origin, normal: unit(linear(f.normal)), x: unit(linear(f.x)) }
}

pub(crate) fn translation(v: Vec3) -> Transform {
    Transform { m: [1.0, 0.0, 0.0, v.x, 0.0, 1.0, 0.0, v.y, 0.0, 0.0, 1.0, v.z] }
}

/// Shapes looked up by [`Shape::is_same`].
#[derive(Default)]
pub(crate) struct ShapeSet(HashMap<u64, Vec<Shape>>);

impl ShapeSet {
    pub(crate) fn insert(&mut self, s: &Shape) {
        let entries = self.0.entry(s.hash()).or_default();
        if !entries.iter().any(|e| e.is_same(s)) {
            entries.push(s.clone());
        }
    }

    /// Adds a shape and its faces, edges and vertices.
    pub(crate) fn insert_all(&mut self, s: &Shape) {
        self.insert(s);
        for kind in [ShapeKind::Face, ShapeKind::Edge, ShapeKind::Vertex] {
            for part in s.sub_shapes(kind) {
                self.insert(&part);
            }
        }
    }

    pub(crate) fn contains(&self, s: &Shape) -> bool {
        self.0.get(&s.hash()).is_some_and(|entries| entries.iter().any(|e| e.is_same(s)))
    }
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
    // Sheets intersected with each other make the curves where they meet.
    let all: Vec<usize> = targets.iter().chain(tools.iter()).copied().collect();
    if matches!(kind, BooleanKind::Intersection)
        && !per_target
        && all.iter().all(|&b| model.bodies[b].body_type == BodyType::Sheet)
        && all.len() >= 2
    {
        let first = vec![model.bodies[all[0]].shape.clone()];
        let rest: Vec<Shape> = all[1..].iter().map(|&b| model.bodies[b].shape.clone()).collect();
        let edges = occt::section_edges(&first, &rest).map_err(|e| error(format!("@opBoolean: {e}")))?;
        model.remove_bodies(&all);
        let info = Info::new(id);
        for wire in occt::connect_edges(&edges).map_err(occt_error)? {
            model.add_body(NewBody::new(wire, BodyType::Wire, info.clone()), &Origins::default(), &info, &|_| None);
        }
        return Ok(Value::UNDEFINED);
    }
    let keep_tools = values::opt_bool(definition, "keepTools")?.unwrap_or(false);
    // Without erasing imprinted edges, faces on one surface stay apart: a jog joins its stretched strip, already
    // split where its bends go, to the rest of the plate this way (batch 013).
    let unify = values::opt_bool(definition, "eraseImprintedEdges")?.unwrap_or(true);
    boolean_bodies_with(model, id, kind, per_target, targets, tools, keep_tools, unify)?;
    Ok(Value::UNDEFINED)
}

/// A boolean of bodies: all of them together, or (`per_target`) the tools with each target separately. The results
/// replace the bodies used.
/// Whether a sheet has a planar face lying in the plane of one of these faces but facing the other way.
fn faces_against(faces: &[Shape], sheet: &Shape) -> bool {
    let planes: Vec<occt::Frame> = faces.iter().filter_map(|f| f.face_plane().ok()).collect();
    sheet.sub_shapes(ShapeKind::Face).iter().filter_map(|f| f.face_plane().ok()).any(|p| {
        planes.iter().any(|q| {
            let n = crate::values::dot(p.normal, q.normal);
            n < -1.0 + 1e-9 && crate::values::dot(crate::values::sub(p.origin, q.origin), q.normal).abs() < 1e-9
        })
    })
}

pub(crate) fn boolean_bodies(
    model: &mut Model,
    id: FeatureId,
    kind: BooleanKind,
    per_target: bool,
    targets: Vec<usize>,
    tools: Vec<usize>,
    keep_tools: bool,
) -> R<()> {
    boolean_bodies_with(model, id, kind, per_target, targets, tools, keep_tools, true)
}

/// As [`boolean_bodies`]; with `unify`, faces and edges left on one surface or curve are merged.
#[allow(clippy::too_many_arguments)]
fn boolean_bodies_with(
    model: &mut Model,
    id: FeatureId,
    kind: BooleanKind,
    per_target: bool,
    targets: Vec<usize>,
    tools: Vec<usize>,
    keep_tools: bool,
    unify: bool,
) -> R<()> {
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
            return Ok(());
        }
        groups.push((inputs.clone(), vec![inputs[0]], inputs[1..].to_vec()));
        consumed.extend(&inputs);
    }

    let mut results = Vec::new();
    for (inputs, objects, group_tools) in &groups {
        // Sheets joined face to face must face the same way to merge: turn over a tool sheet facing against a
        // target it lies on (a sheet metal tab).
        let mut tool_shapes = shapes(group_tools);
        if kind == BooleanKind::Union {
            let object_faces: Vec<Shape> =
                objects.iter().flat_map(|&b| model.bodies[b].shape.sub_shapes(ShapeKind::Face)).collect();
            for tool in tool_shapes.iter_mut() {
                if tool.sub_shapes(ShapeKind::Solid).is_empty() && faces_against(&object_faces, tool) {
                    *tool = tool.reversed();
                }
            }
        }
        let op =
            occt::boolean(kind, &shapes(objects), &tool_shapes, unify).map_err(|e| error(format!("@opBoolean: {e}")))?;
        let input_bodies: Vec<_> = inputs.iter().map(|&b| model.bodies[b].clone()).collect();
        // Faces a tool leaves in the result, such as a pocket's walls, keep the tool's creator, unless the tool is
        // kept: then they are new, made by this operation.
        let mut history_bodies: Vec<&crate::model::Body> = input_bodies.iter().map(|b| &**b).collect();
        let other_tools = group_tools.iter().filter(|t| !inputs.contains(t)).map(|&t| &*model.bodies[t]);
        let mut kept_tool_entities = Vec::new();
        if keep_tools && per_target {
            kept_tool_entities.extend(other_tools.flat_map(|b| b.all_entities()));
        } else {
            history_bodies.extend(other_tools);
        }
        let mut origins = Origins::of_operation(&op, &history_bodies);
        origins.add_derived(&op, &kept_tool_entities);
        let origins = Rc::new(origins);
        let mut claimed = Vec::new();
        // Solids, or for sheets, shells of the faces connected through shared edges.
        let result = op.shape();
        let mut pieces = result.sub_shapes(ShapeKind::Solid);
        if pieces.is_empty() {
            pieces = occt::connected_shells(&result).map_err(occt_error)?;
            if kind == BooleanKind::Union {
                // A union of sheets faces one way throughout, the way the first sheet faced (a sheet metal bend's
                // wrapped face joins its walls this way).
                let reference: Vec<(Vec3, Vec3)> = objects
                    .iter()
                    .flat_map(|&b| model.bodies[b].faces.iter())
                    .filter_map(|f| {
                        let p = f.shape.face_inner_point().ok()?;
                        let (fu, fv) = f.shape.face_fraction(p).ok()?;
                        Some((p, f.shape.face_tangent_plane(fu, fv).ok()?.normal))
                    })
                    .collect();
                for piece in pieces.iter_mut() {
                    let Ok(oriented) = occt::orient_sheet(piece) else { continue };
                    let agrees = reference.iter().find_map(|(p, n)| {
                        let vertex = occt::vertex(*p).ok()?;
                        let face = oriented
                            .sub_shapes(ShapeKind::Face)
                            .into_iter()
                            .find(|f| f.distance(&vertex).is_ok_and(|d| d.distance < 1e-9))?;
                        let (fu, fv) = face.face_fraction(*p).ok()?;
                        Some(crate::values::dot(face.face_tangent_plane(fu, fv).ok()?.normal, *n) > 0.0)
                    });
                    *piece = if agrees == Some(false) { oriented.reversed() } else { oriented };
                }
            }
        }
        for solid in pieces {
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
        let new = NewBody { body_type: BodyType::of(&solid), tid, ..NewBody::replacing(&body, solid) };
        model.add_body(new, &origins, &default, &|_| None);
    }
    Ok(())
}
