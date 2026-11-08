//! Measurements (`@ev*`): the builtins that read a context's geometry. They return std's own types, with units.

use crate::model::{EntityRef, EntityType, Model};
use crate::values::{self, Tags, field, map};
use crate::{GeomBuiltins, model_mut, occt_error, query, regen_error, tags};
use fs_interp::Value;
use fs_interp::builtins::arg;
use fs_interp::eval::{R, error};
use occt::{Box3, Shape, ShapeKind, Vec3};

fn definition(args: &[Value], name: &str) -> R<Value> {
    Ok(arg(args, 1, name)?.clone())
}

/// Runs a measurement over what `key` in the definition finds.
fn measure(
    args: Vec<Value>,
    name: &str,
    key: &str,
    f: impl FnOnce(&Tags, &Model, Vec<EntityRef>) -> R<Value>,
    tags: &Tags,
) -> R<Value> {
    let definition = definition(&args, name)?;
    model_mut(&args, name, |model| {
        let found = query::evaluate(model, field(&definition, key))?;
        f(tags, model, found)
    })
}

fn sum(model: &Model, found: &[EntityRef], f: impl Fn(&occt::Shape) -> occt::Result<f64>) -> R<f64> {
    found.iter().map(|r| f(model.shape(*r)).map_err(occt_error)).sum()
}

pub fn register(m: &mut GeomBuiltins) {
    m.insert("evPlane", |interp, args| {
        let tags = tags(interp)?;
        measure(
            args,
            "evPlane",
            "face",
            |tags, model, found| {
                let face = found.first().filter(|r| r.kind == EntityType::Face);
                let frame = face.and_then(|r| model.shape(*r).face_plane().ok());
                frame
                    .map(|f| tags.plane(occt::Frame { x: values::canonical_x(f.normal), ..f }))
                    .ok_or_else(|| regen_error("CANNOT_RESOLVE_PLANE"))
            },
            &tags,
        )
    });
    m.insert("evOwnerSketchPlane", |interp, args| {
        let tags = tags(interp)?;
        measure(
            args,
            "evOwnerSketchPlane",
            "entity",
            |tags, model, found| {
                let plane = found.first().and_then(|r| model.body(*r).sketch_plane);
                plane.map(|f| tags.plane(f)).ok_or_else(|| regen_error("CANNOT_RESOLVE_SKETCH_PLANE"))
            },
            &tags,
        )
    });
    m.insert("evVertexPoint", |interp, args| {
        let tags = tags(interp)?;
        measure(
            args,
            "evVertexPoint",
            "vertex",
            |tags, model, found| {
                let vertex =
                    found.first().filter(|r| r.kind == EntityType::Vertex || model.body(**r).vertices.len() == 1);
                let point = vertex.and_then(|r| {
                    let shape = match r.kind {
                        EntityType::Vertex => model.shape(*r).clone(),
                        _ => model.body(*r).vertices[0].shape.clone(),
                    };
                    shape.vertex_point().ok()
                });
                point.map(|p| tags.length_vector(p)).ok_or_else(|| regen_error("CANNOT_RESOLVE_ENTITIES"))
            },
            &tags,
        )
    });
    m.insert("evArea", |interp, args| {
        let tags = tags(interp)?;
        measure(
            args,
            "evArea",
            "faces",
            |tags, model, found| Ok(tags.with_units(sum(model, &found, |s| s.area())?, "meter", 2.0)),
            &tags,
        )
    });
    m.insert("evLength", |interp, args| {
        let tags = tags(interp)?;
        measure(
            args,
            "evLength",
            "edges",
            |tags, model, found| Ok(tags.length(sum(model, &found, |s| s.length())?)),
            &tags,
        )
    });
    m.insert("evVolume", |interp, args| {
        let tags = tags(interp)?;
        measure(
            args,
            "evVolume",
            "bodies",
            |tags, model, found| Ok(tags.with_units(sum(model, &found, |s| s.volume())?, "meter", 3.0)),
            &tags,
        )
    });
    m.insert("evBox", |interp, args| {
        let tags = tags(interp)?;
        let definition = definition(&args, "evBox")?;
        let tight = definition.get("tight").and_then(Value::as_bool).unwrap_or(true);
        // A CoordSystem: the box is in its coordinates.
        let frame = match field(&definition, "cSys") {
            v if v.is_undefined() => None,
            v => Some(occt::Frame {
                origin: values::vec3(field(v, "origin"), "evBox3d cSys")?,
                normal: values::vec3(field(v, "zAxis"), "evBox3d cSys")?,
                x: values::vec3(field(v, "xAxis"), "evBox3d cSys")?,
            }),
        };
        measure(
            args,
            "evBox",
            "topology",
            |tags, model, found| {
                let mut total: Option<Box3> = None;
                for r in &found {
                    let shape = model.shape(*r);
                    let b = match frame {
                        Some(frame) => shape.bounding_box_in(frame, tight),
                        None => shape.bounding_box(tight),
                    }
                    .map_err(occt_error)?;
                    if b.empty {
                        continue;
                    }
                    total = Some(match total {
                        None => b,
                        Some(t) => Box3 {
                            min: Vec3::new(t.min.x.min(b.min.x), t.min.y.min(b.min.y), t.min.z.min(b.min.z)),
                            max: Vec3::new(t.max.x.max(b.max.x), t.max.y.max(b.max.y), t.max.z.max(b.max.z)),
                            empty: false,
                        },
                    });
                }
                let total = total.ok_or_else(|| regen_error("CANNOT_RESOLVE_ENTITIES"))?;
                Ok(crate::values::map(vec![
                    ("minCorner", tags.length_vector(total.min)),
                    ("maxCorner", tags.length_vector(total.max)),
                ]))
            },
            &tags,
        )
    });
    m.insert("evAxis", |interp, args| {
        let tags = tags(interp)?;
        measure(
            args,
            "evAxis",
            "axis",
            |tags, model, found| {
                // A sketch point's axis runs through it along its sketch's normal, and a mate connector's along its z
                // axis.
                if let [r] = found[..] {
                    let body = model.body(r);
                    let point = r.kind == EntityType::Vertex || body.body_type == crate::model::BodyType::Point;
                    if let (true, Some(plane)) = (point, body.sketch_plane) {
                        let at = model.shape(r).sub_shapes(occt::ShapeKind::Vertex);
                        let at = at.first().and_then(|v| v.vertex_point().ok());
                        let at = at.ok_or_else(|| regen_error("CANNOT_RESOLVE_ENTITIES"))?;
                        return Ok(tags.line(at, plane.normal));
                    }
                    if let (EntityType::Body, Some(frame)) = (r.kind, body.frame) {
                        return Ok(tags.line(frame.origin, frame.normal));
                    }
                }
                let shape = single_entity(model, &found).ok_or_else(|| regen_error("CANNOT_RESOLVE_ENTITIES"))?;
                let axis = shape.axis().map_err(|_| regen_error("CANNOT_RESOLVE_ENTITIES"))?;
                Ok(tags.line(axis.origin, axis.direction))
            },
            &tags,
        )
    });
    m.insert("evLine", |interp, args| {
        let tags = tags(interp)?;
        measure(
            args,
            "evLine",
            "edge",
            |tags, model, found| {
                let edge = single_edge(model, &found).ok_or_else(|| regen_error("INVALID_INPUT"))?;
                if edge.curve_type().ok() != Some(occt::CurveType::Line) {
                    return Err(regen_error("INVALID_INPUT"));
                }
                let axis = edge.axis().map_err(|_| regen_error("INVALID_INPUT"))?;
                Ok(tags.line(axis.origin, axis.direction))
            },
            &tags,
        )
    });
    m.insert("evEdgeTangentLines", |interp, args| {
        let tags = tags(interp)?;
        let definition = definition(&args, "evEdgeTangentLines")?;
        let parameters = values::numbers(field(&definition, "parameters"), "evEdgeTangentLines parameters")?;
        let arc_length = values::opt_bool(&definition, "arcLengthParameterization")?.unwrap_or(true);
        if !field(&definition, "face").is_undefined() {
            return Err(error("@evEdgeTangentLines: face isn't supported yet"));
        }
        measure(
            args,
            "evEdgeTangentLines",
            "edge",
            |tags, model, found| {
                let edge = single_edge(model, &found).ok_or_else(|| regen_error("NO_TANGENT_LINE"))?;
                let mut lines = Vec::new();
                for t in parameters {
                    let tangent = edge.edge_tangent(t, arc_length).map_err(|_| regen_error("NO_TANGENT_LINE"))?;
                    lines.push(tags.line(tangent.origin, tangent.direction));
                }
                Ok(Value::array(lines))
            },
            &tags,
        )
    });
    m.insert("evDistance", |interp, args| {
        let tags = tags(interp)?;
        let definition = definition(&args, "evDistance")?;
        if values::opt_bool(&definition, "maximum")?.unwrap_or(false) {
            return Err(error("@evDistance: maximum isn't supported yet"));
        }
        let arc_length = values::opt_bool(&definition, "arcLengthParameterization")?.unwrap_or(true);
        model_mut(&args, "evDistance", |model| distance(&tags, model, &definition, arc_length))
    });
    m.insert("evCurveDefinition", |interp, args| {
        let tags = tags(interp)?;
        measure(
            args,
            "evCurveDefinition",
            "edge",
            |tags, model, found| {
                let r = found.first().ok_or_else(|| regen_error("CANNOT_RESOLVE_ENTITIES"))?;
                if r.kind != EntityType::Edge {
                    return Err(regen_error("INVALID_INPUT"));
                }
                let conic = model.shape(*r).curve_definition().map_err(occt_error)?;
                match conic.kind {
                    0 => Ok(tags.line(conic.frame.origin, conic.frame.normal)),
                    1 => tags.tagged(
                        map(vec![
                            ("coordSystem", tags.coord_system(conic.frame)?),
                            ("radius", tags.length(conic.radius1)),
                        ]),
                        "Circle",
                    ),
                    2 => tags.tagged(
                        map(vec![
                            ("coordSystem", tags.coord_system(conic.frame)?),
                            ("majorRadius", tags.length(conic.radius1)),
                            ("minorRadius", tags.length(conic.radius2)),
                        ]),
                        "Ellipse",
                    ),
                    // B-splines and the rest: std only promises a map.
                    _ => Ok(map(vec![("curveType", Value::string("OTHER"))])),
                }
            },
            &tags,
        )
    });
    m.insert("evSurfaceDefinition", |interp, args| {
        let tags = tags(interp)?;
        measure(
            args,
            "evSurfaceDefinition",
            "face",
            |tags, model, found| {
                let r = found.first().ok_or_else(|| regen_error("CANNOT_RESOLVE_ENTITIES"))?;
                if r.kind != EntityType::Face {
                    return Err(regen_error("INVALID_INPUT"));
                }
                let s = model.shape(*r).surface_definition().map_err(occt_error)?;
                let cs = || tags.coord_system(s.frame);
                match s.kind {
                    0 => {
                        let plane = model.shape(*r).face_plane().map_err(occt_error)?;
                        Ok(tags.plane(occt::Frame { x: values::canonical_x(plane.normal), ..plane }))
                    }
                    1 => tags.tagged(map(vec![("coordSystem", cs()?), ("radius", tags.length(s.radius1))]), "Cylinder"),
                    2 => tags.tagged(
                        map(vec![("coordSystem", cs()?), ("halfAngle", tags.with_units(s.radius1, "radian", 1.0))]),
                        "Cone",
                    ),
                    3 => tags.tagged(map(vec![("coordSystem", cs()?), ("radius", tags.length(s.radius1))]), "Sphere"),
                    4 => tags.tagged(
                        map(vec![
                            ("coordSystem", cs()?),
                            ("radius", tags.length(s.radius1)),
                            ("minorRadius", tags.length(s.radius2)),
                        ]),
                        "Torus",
                    ),
                    _ => Ok(map(vec![("surfaceType", Value::string("OTHER"))])),
                }
            },
            &tags,
        )
    });
    // The frame a face on this plane would have: Onshape's canonical x direction.
    m.insert("alignCanonically", |interp, args| {
        let tags = tags(interp)?;
        let definition = definition(&args, "alignCanonically")?;
        let plane = values::plane(field(&definition, "plane"), "alignCanonically plane")?;
        Ok(tags.plane(occt::Frame { x: values::canonical_x(plane.normal), ..plane }))
    });
    m.insert("evFaceTangentPlanes", |interp, args| {
        let tags = tags(interp)?;
        let definition = definition(&args, "evFaceTangentPlanes")?;
        let parameters = field(&definition, "parameters")
            .as_array()
            .ok_or_else(|| error("@evFaceTangentPlanes: parameters must be an array"))?
            .iter()
            .map(|uv| values::vec2(uv, "evFaceTangentPlanes parameter"))
            .collect::<R<Vec<_>>>()?;
        measure(
            args,
            "evFaceTangentPlanes",
            "face",
            |tags, model, found| {
                let r = found
                    .first()
                    .filter(|r| r.kind == EntityType::Face)
                    .ok_or_else(|| regen_error("NO_TANGENT_PLANE"))?;
                let face = model.shape(*r);
                let mut planes = Vec::new();
                for (u, v) in parameters {
                    let frame = face.face_tangent_plane(u, v).map_err(|_| regen_error("NO_TANGENT_PLANE"))?;
                    planes.push(tags.plane(frame));
                }
                Ok(Value::array(planes))
            },
            &tags,
        )
    });
}

/// The one entity a query found, or the one edge of a body that has only one edge (a sketch line, say).
fn single_entity(model: &Model, found: &[EntityRef]) -> Option<Shape> {
    let r = found.first()?;
    if r.kind == EntityType::Body {
        let body = model.body(*r);
        return match (body.faces.len(), body.edges.len()) {
            (0, 1) => Some(body.edges[0].shape.clone()),
            (1, _) => Some(body.faces[0].shape.clone()),
            _ => None,
        };
    }
    Some(model.shape(*r).clone())
}

fn single_edge(model: &Model, found: &[EntityRef]) -> Option<Shape> {
    single_entity(model, found).filter(|s| s.kind() == Some(ShapeKind::Edge))
}

/// What one side of `@evDistance` holds: shapes to measure from, and how to describe a point on each.
enum Side {
    /// Query results, in order, and whether they were extended.
    Entities(Vec<(EntityType, Shape)>, bool),
    /// Points, lines or planes, as shapes that stand in for them.
    Points,
}

const FAR: f64 = 1e4;

/// One side of `@evDistance`. With `extend`, bodies are left out and edges and faces are extended to their
/// underlying lines, planes and closed curves.
fn side(model: &Model, v: &Value, what: &str, extend: bool) -> R<(Side, Vec<Shape>)> {
    if v.get("queryType").is_some() {
        let found = query::evaluate(model, v)?;
        let mut entities: Vec<(EntityType, Shape)> = Vec::new();
        let mut shapes = Vec::new();
        for r in &found {
            if extend && r.kind == EntityType::Body {
                continue;
            }
            let shape = model.shape(*r);
            shapes.push(if extend { shape.extended(FAR).map_err(occt_error)? } else { shape.clone() });
            entities.push((r.kind, shape.clone()));
        }
        return Ok((Side::Entities(entities, extend), shapes));
    }
    // An array of points, lines or planes; a point is itself an array, of lengths (maps with a value and unit).
    let is_item = |x: &Value| x.as_array().is_some() || x.get("origin").is_some();
    let items: Vec<Value> = match v.as_array() {
        Some(items) if items.first().is_some_and(is_item) => items.iter().cloned().collect(),
        _ => vec![v.clone()],
    };
    let mut shapes = Vec::new();
    for item in &items {
        if item.get("direction").is_some() {
            let line = values::line(item, what)?;
            let a = values::sub(line.origin, values::scale(line.direction, FAR));
            let b = values::add(line.origin, values::scale(line.direction, FAR));
            shapes.push(occt::line_edge(a, b).map_err(occt_error)?);
        } else if item.get("normal").is_some() {
            let frame = values::plane(item, what)?;
            shapes.push(occt::plane_face(frame, -FAR, FAR, -FAR, FAR).map_err(occt_error)?);
        } else {
            let p = values::vec3(item, what)?;
            shapes.push(occt::vertex(p).map_err(occt_error)?);
        }
    }
    Ok((Side::Points, shapes))
}

fn distance(tags: &Tags, model: &Model, definition: &Value, arc_length: bool) -> R<Value> {
    let extend = |key| values::opt_bool(definition, key).map(|b| b.unwrap_or(false));
    let (side0, shapes0) = side(model, field(definition, "side0"), "evDistance side0", extend("extendSide0")?)?;
    let (side1, shapes1) = side(model, field(definition, "side1"), "evDistance side1", extend("extendSide1")?)?;
    let mut best: Option<(f64, usize, usize, occt::Distance)> = None;
    for (i, a) in shapes0.iter().enumerate() {
        for (j, b) in shapes1.iter().enumerate() {
            let d = a.distance(b).map_err(occt_error)?;
            if best.as_ref().is_none_or(|(min, ..)| d.distance < *min - 1e-12) {
                best = Some((d.distance, i, j, d));
            }
        }
    }
    let (_, i, j, d) = best.ok_or_else(|| regen_error("CANNOT_RESOLVE_ENTITIES"))?;
    let describe = |side: &Side, index: usize, point: Vec3| -> R<Value> {
        let parameter = match side {
            Side::Entities(entities, extended) => match &entities[index] {
                (EntityType::Edge, edge) if *extended => {
                    Value::number(edge.edge_fraction_extended(point).map_err(occt_error)?)
                }
                (EntityType::Edge, edge) => Value::number(edge.edge_fraction(point, arc_length).map_err(occt_error)?),
                (EntityType::Face, face) => {
                    let (u, v) = face.face_fraction(point).map_err(occt_error)?;
                    tags.vector2(u, v)
                }
                _ => Value::number(0.0),
            },
            Side::Points => Value::number(0.0),
        };
        Ok(map(vec![
            ("point", tags.length_vector(point)),
            ("index", Value::number(index as f64)),
            ("parameter", parameter),
        ]))
    };
    let sides = vec![describe(&side0, i, d.a)?, describe(&side1, j, d.b)?];
    tags.tagged(map(vec![("distance", tags.length(d.distance)), ("sides", Value::array(sides))]), "DistanceResult")
}
