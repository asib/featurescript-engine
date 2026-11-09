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

/// How a tool and a target meet (`ClashType`), or `None` if they don't.
fn clash(tool: &Shape, target: &Shape) -> Option<&'static str> {
    if tool.distance(target).ok()?.distance > 1e-8 {
        return None;
    }
    let solid = |s: &Shape| !s.sub_shapes(ShapeKind::Solid).is_empty();
    if !(solid(tool) && solid(target)) {
        return Some("INTERFERE");
    }
    let common =
        occt::boolean(occt::BooleanKind::Intersection, std::slice::from_ref(target), std::slice::from_ref(tool), false)
            .ok()
            .and_then(|op| op.shape().volume().ok())
            .unwrap_or(0.0);
    let (tool_volume, target_volume) = (tool.volume().unwrap_or(0.0), target.volume().unwrap_or(0.0));
    let same = |a: f64, b: f64| (a - b).abs() <= 1e-9 * b.abs().max(1e-12);
    Some(if common <= 1e-15 {
        "ABUT_NO_CLASS"
    } else if same(common, target_volume) {
        "TARGET_IN_TOOL"
    } else if same(common, tool_volume) {
        "TOOL_IN_TARGET"
    } else {
        "INTERFERE"
    })
}

/// What `@clusterBodies` compares: topology counts, then sizes that don't change when a body turns.
fn body_signature(model: &Model, b: usize) -> R<(Vec<usize>, Vec<f64>)> {
    let body = &model.bodies[b];
    let counts = vec![body.faces.len(), body.edges.len(), body.vertices.len()];
    let shape = &body.shape;
    let moments = shape.principal_moments().map_err(occt_error)?;
    let sizes = vec![
        shape.volume().unwrap_or(0.0),
        shape.area().unwrap_or(0.0),
        shape.length().unwrap_or(0.0),
        moments.x,
        moments.y,
        moments.z,
    ];
    Ok((counts, sizes))
}

/// The box `evPlane` takes the middle of: the face's box in the plane's axes, but on each side where the face only
/// touches it at a point (a vertex, or where a curve turns) rather than along a straight edge, 0.6% of the box's
/// size further out. So it is in Onshape: a filled right triangle 30 × 12 mm with its legs along the axes has its
/// plane's origin at (15.09, 6.036), not (15, 6); a ruled trapezoid, an annular sector swept along an arc, and so
/// on (batches 004 c034, 024 c008–c010, to 1e-7).
fn plane_origin_box(face: &occt::Shape, axes: occt::Frame, b: occt::Box3) -> occt::Box3 {
    let y = values::cross(axes.normal, axes.x);
    let local = |p: occt::Vec3| {
        let d = values::sub(p, axes.origin);
        (values::dot(d, axes.x), values::dot(d, y))
    };
    let size = (b.max.x - b.min.x, b.max.y - b.min.y);
    // The box can stand off the face by its tolerance (1e-7 m).
    let tolerance = 1e-6;
    // Whether an edge runs along the side where coordinate `axis` is `at` (a straight one, whatever its curve:
    // ruled and swept sheets' straight edges are B-splines).
    let along = |axis: usize, at: f64| {
        face.sub_shapes(occt::ShapeKind::Edge).iter().any(|e| {
            let points: Vec<(f64, f64)> = (0..=8)
                .filter_map(|i| e.edge_tangent(i as f64 / 8.0, false).ok().map(|t| local(t.origin)))
                .map(|p| if axis == 0 { p } else { (p.1, p.0) })
                .collect();
            points.len() == 9
                && points.iter().all(|p| (p.0 - at).abs() < tolerance)
                && (points[0].1 - points[8].1).abs() > tolerance
        })
    };
    let grow = 0.006;
    let mut out = b;
    if !along(0, b.min.x) {
        out.min.x -= grow * size.0;
    }
    if !along(0, b.max.x) {
        out.max.x += grow * size.0;
    }
    if !along(1, b.min.y) {
        out.min.y -= grow * size.1;
    }
    if !along(1, b.max.y) {
        out.max.y += grow * size.1;
    }
    out
}

pub fn register(m: &mut GeomBuiltins) {
    // Bodies alike up to a rotation, by topology counts, volume, area, length and principal moments of inertia.
    m.insert("clusterBodies", |_, args| {
        let definition = definition(&args, "clusterBodies")?;
        let tolerance = field(&definition, "relativeTolerance").as_number().unwrap_or(1e-6);
        model_mut(&args, "clusterBodies", |model| {
            let bodies: Vec<usize> = query::evaluate(model, field(&definition, "bodies"))?
                .into_iter()
                .filter(|r| r.kind == EntityType::Body)
                .map(|r| r.body)
                .collect();
            let signatures = bodies.iter().map(|&b| body_signature(model, b)).collect::<R<Vec<_>>>()?;
            let alike = |a: &(Vec<usize>, Vec<f64>), b: &(Vec<usize>, Vec<f64>)| {
                // Moments are compared against the largest, so a near-zero one doesn't demand an exact match.
                let largest = a.1[3..].iter().chain(&b.1[3..]).fold(0.0f64, |m, x| m.max(x.abs()));
                a.0 == b.0
                    && a.1.iter().zip(&b.1).enumerate().all(|(i, (x, y))| {
                        let scale = if i >= 3 { largest } else { x.abs().max(y.abs()) };
                        (x - y).abs() <= tolerance * scale + 1e-15
                    })
            };
            let mut clusters: Vec<Vec<usize>> = Vec::new();
            for (i, signature) in signatures.iter().enumerate() {
                match clusters.iter_mut().find(|c| alike(&signatures[c[0]], signature)) {
                    Some(cluster) => cluster.push(i),
                    None => clusters.push(vec![i]),
                }
            }
            Ok(Value::array(
                clusters
                    .into_iter()
                    .map(|c| Value::array(c.into_iter().map(|i| Value::number(i as f64)).collect()))
                    .collect(),
            ))
        })
    });
    m.insert("evCollisionDetection", |interp, args| {
        let tags = tags(interp)?;
        let definition = definition(&args, "evCollisionDetection")?;
        let clash_type = fs_interp::types::find("ClashType").ok_or_else(|| error("std type ClashType isn't loaded"))?;
        model_mut(&args, "evCollisionDetection", |model| {
            let tools = query::evaluate(model, field(&definition, "tools"))?;
            let targets = query::evaluate(model, field(&definition, "targets"))?;
            let shape = |r: EntityRef| {
                if r.kind == EntityType::Body { model.bodies[r.body].shape.clone() } else { model.shape(r).clone() }
            };
            let transient = |r: EntityRef| tags.transient_query(&query::tid_string(model.tid(r)));
            let owner = |r: EntityRef| tags.transient_query(&query::tid_string(model.bodies[r.body].tid));
            // Bodies clash face by face: each pair of their faces that meet is reported, with how the bodies
            // meet (std's sheet metal booleans find the walls a tool cuts from the target faces, batch 017).
            // Bodies that meet without their faces touching (one inside the other) are reported as bodies.
            let faces_of = |r: EntityRef| -> Vec<EntityRef> {
                if r.kind != EntityType::Body || model.bodies[r.body].body_type != crate::model::BodyType::Solid {
                    return vec![r];
                }
                (0..model.bodies[r.body].faces.len())
                    .map(|index| EntityRef { body: r.body, kind: EntityType::Face, index })
                    .collect()
            };
            let mut out = Vec::new();
            let mut push = |kind: &str, tool: EntityRef, target: EntityRef| {
                out.push(map(vec![
                    ("type", tags.enum_value(clash_type, kind)),
                    ("tool", transient(tool)),
                    ("toolBody", owner(tool)),
                    ("target", transient(target)),
                    ("targetBody", owner(target)),
                ]));
            };
            for &tool in &tools {
                for &target in &targets {
                    let Some(kind) = clash(&shape(tool), &shape(target)) else { continue };
                    let (tool_faces, target_faces) = (faces_of(tool), faces_of(target));
                    if tool_faces == [tool] && target_faces == [target] {
                        push(kind, tool, target);
                        continue;
                    }
                    let mut touched = false;
                    for &tf in &tool_faces {
                        for &gf in &target_faces {
                            if shape(tf).distance(&shape(gf)).is_ok_and(|d| d.distance <= 1e-8) {
                                push(kind, tf, gf);
                                touched = true;
                            }
                        }
                    }
                    if !touched {
                        push(kind, tool, target);
                    }
                }
            }
            Ok(Value::array(out))
        })
    });
    // The centroid of entities together, each weighted by its volume, area or length (an internal end cap's
    // drive point, batch 019).
    m.insert("evApproximateCentroid", |interp, args| {
        let tags = tags(interp)?;
        measure(
            args,
            "evApproximateCentroid",
            "entities",
            |tags, model, found| {
                let mut total = 0.0;
                let mut sum = Vec3::new(0.0, 0.0, 0.0);
                for r in &found {
                    let shape = if r.kind == EntityType::Body {
                        model.bodies[r.body].shape.clone()
                    } else {
                        model.shape(*r).clone()
                    };
                    let weight = match r.kind {
                        EntityType::Face => shape.area().unwrap_or(0.0),
                        EntityType::Edge => shape.length().unwrap_or(0.0),
                        EntityType::Vertex => 1.0,
                        EntityType::Body => {
                            shape.volume().ok().filter(|v| *v > 0.0).or(shape.area().ok()).unwrap_or(1.0)
                        }
                    };
                    let c = match r.kind {
                        EntityType::Vertex => shape.vertex_point().map_err(occt_error)?,
                        _ => shape.centroid().map_err(occt_error)?,
                    };
                    sum = values::add(sum, values::scale(c, weight));
                    total += weight;
                }
                if total <= 0.0 {
                    return Err(regen_error("CANNOT_RESOLVE_ENTITIES"));
                }
                Ok(tags.length_vector(values::scale(sum, 1.0 / total)))
            },
            &tags,
        )
    });
    m.insert("evPlane", |interp, args| {
        let tags = tags(interp)?;
        measure(
            args,
            "evPlane",
            "face",
            |tags, model, found| {
                let face = found.first().filter(|r| r.kind == EntityType::Face);
                let frame = face.and_then(|r| model.shape(*r).face_plane().ok());
                let frame = frame.map(|f| occt::Frame { x: values::canonical_x(f.normal), ..f });
                // The origin in the middle of the face's box in the plane's own axes: the frame Onshape gave the
                // face's surface if it's known (a ruled sheet from a curved edge, batch 040), else the canonical.
                let frame = match (face, frame) {
                    (Some(r), Some(f)) => {
                        let axes = match model.info(*r).surface_frame {
                            Some(s) if values::norm(values::cross(s.normal, f.normal)) < 1e-9 => {
                                occt::Frame { x: s.x, ..f }
                            }
                            _ => f,
                        };
                        model.shape(*r).bounding_box_in(axes, true).ok().map(|b| {
                            let y = values::cross(axes.normal, axes.x);
                            let b = plane_origin_box(model.shape(*r), axes, b);
                            let (u, v) = ((b.min.x + b.max.x) / 2.0, (b.min.y + b.max.y) / 2.0);
                            occt::Frame {
                                origin: values::add(
                                    axes.origin,
                                    values::add(values::scale(axes.x, u), values::scale(y, v)),
                                ),
                                ..f
                            }
                        })
                    }
                    (_, f) => f,
                };
                // Nothing to resolve is its own error (batch 060 c009, c010); a first entity that isn't a plane
                // is CANNOT_RESOLVE_PLANE.
                if found.is_empty() {
                    return Err(regen_error("CANNOT_RESOLVE_ENTITIES"));
                }
                frame.map(|f| tags.plane(f)).ok_or_else(|| regen_error("CANNOT_RESOLVE_PLANE"))
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
                    // A spline fitted through two points is straight (a jog's second bend line, batch 013).
                    let (origin, direction) = straight_spline(&edge).ok_or_else(|| regen_error("INVALID_INPUT"))?;
                    return Ok(tags.line(origin, direction));
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
        let face_query = field(&definition, "face").clone();
        measure(
            args,
            "evEdgeTangentLines",
            "edge",
            |tags, model, found| {
                let edge = single_edge(model, &found).ok_or_else(|| regen_error("NO_TANGENT_LINE"))?;
                // With a face, the edge runs the way the face's boundary uses it: with the face on its left.
                let mut reversed = false;
                if !face_query.is_undefined() {
                    let faces = query::evaluate(model, &face_query)?;
                    let face = faces
                        .iter()
                        .find(|r| r.kind == EntityType::Face)
                        .ok_or_else(|| regen_error("NO_TANGENT_LINE"))?;
                    let used = model.shape(*face).sub_shapes(ShapeKind::Edge);
                    let used = used.iter().find(|e| e.is_same(&edge)).ok_or_else(|| regen_error("NO_TANGENT_LINE"))?;
                    reversed = used.is_reversed() != edge.is_reversed();
                }
                let mut lines = Vec::new();
                for t in parameters {
                    let t = if reversed { 1.0 - t } else { t };
                    let tangent = edge.edge_tangent(t, arc_length).map_err(|_| regen_error("NO_TANGENT_LINE"))?;
                    let direction = if reversed { values::scale(tangent.direction, -1.0) } else { tangent.direction };
                    lines.push(tags.line(tangent.origin, direction));
                }
                Ok(Value::array(lines))
            },
            &tags,
        )
    });
    m.insert("evEdgeConvexity", |interp, args| {
        let tags = tags(interp)?;
        let convexity = fs_interp::types::find("EdgeConvexityType")
            .ok_or_else(|| error("std type EdgeConvexityType isn't loaded"))?;
        measure(
            args,
            "evEdgeConvexity",
            "edge",
            |tags, model, found| {
                let edge = match found[..] {
                    [r] if r.kind == EntityType::Edge => r,
                    [_, _, ..] => return Err(regen_error("TOO_MANY_ENTITIES_SELECTED")),
                    _ => return Err(regen_error("BAD_GEOMETRY")),
                };
                let kind =
                    crate::selection::convexity(model, edge, false).ok_or_else(|| regen_error("BAD_GEOMETRY"))?;
                Ok(tags.enum_value(convexity, kind))
            },
            &tags,
        )
    });
    m.insert("evEdgeCurvatures", |interp, args| {
        let tags = tags(interp)?;
        let definition = definition(&args, "evEdgeCurvatures")?;
        let parameters = values::numbers(field(&definition, "parameters"), "evEdgeCurvatures parameters")?;
        let arc_length = values::opt_bool(&definition, "arcLengthParameterization")?.unwrap_or(true);
        let face_query = field(&definition, "face").clone();
        model_mut(&args, "evEdgeCurvatures", |model| {
            let found = query::evaluate(model, field(&definition, "edge"))?;
            let edge = single_edge(model, &found).ok_or_else(|| regen_error("NO_TANGENT_LINE"))?;
            // With a face, the edge runs the way the face's boundary uses it: with the face on its left.
            let mut reversed = false;
            if !face_query.is_undefined() {
                let faces = query::evaluate(model, &face_query)?;
                let face =
                    faces.iter().find(|r| r.kind == EntityType::Face).ok_or_else(|| regen_error("NO_TANGENT_LINE"))?;
                let used = model.shape(*face).sub_shapes(ShapeKind::Edge);
                let used = used.iter().find(|e| e.is_same(&edge)).ok_or_else(|| regen_error("NO_TANGENT_LINE"))?;
                reversed = used.is_reversed() != edge.is_reversed();
            }
            let mut out = Vec::new();
            for t in parameters {
                let t = if reversed { 1.0 - t } else { t };
                let c = edge.edge_curvature(t, arc_length).map_err(|_| regen_error("NO_TANGENT_LINE"))?;
                let tangent = if reversed { values::scale(c.frame.normal, -1.0) } else { c.frame.normal };
                let frame = occt::Frame { origin: c.frame.origin, normal: tangent, x: c.frame.x };
                let curvature = tags.with_units(c.radius1, "meter", -1.0);
                out.push(tags.tagged(
                    map(vec![("frame", tags.coord_system(frame)?), ("curvature", curvature)]),
                    "EdgeCurvatureResult",
                )?);
            }
            Ok(Value::array(out))
        })
    });
    m.insert("evEdgeCurvatureDerivatives", |interp, args| {
        let tags = tags(interp)?;
        let definition = definition(&args, "evEdgeCurvatureDerivatives")?;
        let parameters = values::numbers(field(&definition, "parameters"), "evEdgeCurvatureDerivatives parameters")?;
        let arc_length = values::opt_bool(&definition, "arcLengthParameterization")?.unwrap_or(true);
        measure(
            args,
            "evEdgeCurvatureDerivatives",
            "edge",
            |tags, model, found| {
                let edge = single_edge(model, &found).ok_or_else(|| regen_error("NO_TANGENT_LINE"))?;
                let mut out = Vec::new();
                for t in parameters {
                    let k =
                        edge.edge_curvature_derivative(t, arc_length).map_err(|_| regen_error("NO_TANGENT_LINE"))?;
                    let per_area = |x: f64| tags.with_units(x, "meter", -2.0);
                    out.push(
                        Value::array(vec![per_area(k.x), per_area(k.y), per_area(k.z)]).with_tag(Some(tags.vector)),
                    );
                }
                Ok(Value::array(out))
            },
            &tags,
        )
    });
    m.insert("evFaceTangentPlanesAtEdge", |interp, args| {
        let tags = tags(interp)?;
        let definition = definition(&args, "evFaceTangentPlanesAtEdge")?;
        let parameters = values::numbers(field(&definition, "parameters"), "evFaceTangentPlanesAtEdge parameters")?;
        let arc_length = values::opt_bool(&definition, "arcLengthParameterization")?.unwrap_or(true);
        let face_orientation = values::opt_bool(&definition, "usingFaceOrientation")?.unwrap_or(false);
        model_mut(&args, "evFaceTangentPlanesAtEdge", |model| {
            let edges = query::evaluate(model, field(&definition, "edge"))?;
            let edge = single_edge(model, &edges).ok_or_else(|| regen_error("NO_TANGENT_PLANE"))?;
            let faces = query::evaluate(model, field(&definition, "face"))?;
            let face =
                faces.first().filter(|r| r.kind == EntityType::Face).ok_or_else(|| regen_error("NO_TANGENT_PLANE"))?;
            let face = model.shape(*face).clone();
            // With the face's orientation, the edge runs with the face on its left.
            let reversed = face_orientation && {
                let used = face.sub_shapes(ShapeKind::Edge);
                let used = used.iter().find(|e| e.is_same(&edge)).ok_or_else(|| regen_error("NO_TANGENT_PLANE"))?;
                used.is_reversed() != edge.is_reversed()
            };
            let mut planes = Vec::new();
            for t in parameters {
                let t = if reversed { 1.0 - t } else { t };
                let tangent = edge.edge_tangent(t, arc_length).map_err(|_| regen_error("NO_TANGENT_PLANE"))?;
                let direction = if reversed { values::scale(tangent.direction, -1.0) } else { tangent.direction };
                let (fu, fv) = face.face_fraction(tangent.origin).map_err(|_| regen_error("NO_TANGENT_PLANE"))?;
                let normal = face.face_tangent_plane(fu, fv).map_err(|_| regen_error("NO_TANGENT_PLANE"))?.normal;
                let x = values::sub(direction, values::scale(normal, values::dot(direction, normal)));
                let x = values::normalize(x).ok_or_else(|| regen_error("NO_TANGENT_PLANE"))?;
                planes.push(tags.plane(occt::Frame { origin: tangent.origin, normal, x }));
            }
            Ok(Value::array(planes))
        })
    });
    // Pairs of planar faces a thin solid's thickness apart, facing away from each other with the material between
    // (sheet metal recognition, batch 031): one group per body, the faces of one side in `side0` and their partners
    // in `side1`, side0 being the faces that join the body's first paired face across shared edges.
    m.insert("evOffsetDetection", |_, args| {
        let definition = definition(&args, "evOffsetDetection")?;
        model_mut(&args, "evOffsetDetection", |model| {
            let bodies: Vec<usize> =
                query::evaluate(model, field(&definition, "bodies"))?.iter().map(|r| r.body).collect();
            let mut groups = Vec::new();
            for b in bodies {
                if let Some(group) = offset_group(model, b)? {
                    groups.push(group);
                }
            }
            Ok(Value::array(groups))
        })
    });
    m.insert("evDistance", |interp, args| {
        let tags = tags(interp)?;
        let definition = definition(&args, "evDistance")?;
        let arc_length = values::opt_bool(&definition, "arcLengthParameterization")?.unwrap_or(true);
        model_mut(&args, "evDistance", |model| distance(&tags, model, &definition, arc_length))
    });
    // Whether a face wraps round its surface each way (`trimmed`, the default) or its surface does (batch 072
    // c001: a tube swept along an arc, periodic round the tube).
    m.insert("evFacePeriodicity", |_, args| {
        let definition = definition(&args, "evFacePeriodicity")?;
        let trimmed = values::opt_bool(&definition, "trimmed")?.unwrap_or(true);
        model_mut(&args, "evFacePeriodicity", |model| {
            let r = query::evaluate(model, field(&definition, "face"))?
                .into_iter()
                .find(|r| r.kind == EntityType::Face)
                .ok_or_else(|| regen_error("CANNOT_RESOLVE_ENTITIES"))?;
            let face = model.shape(r).clone();
            let ((u0, v0), (u1, v1)) = face.face_uv_bounds().map_err(occt_error)?;
            let full = |a: f64, b: f64| (b - a) >= 2.0 * std::f64::consts::PI - 1e-9;
            let each = |along_u: bool, a: f64, b: f64| occt::face_periodic(&face, along_u) && (!trimmed || full(a, b));
            Ok(Value::array(vec![Value::bool(each(true, u0, u1)), Value::bool(each(false, v0, v1))]))
        })
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
                    _ if matches!(
                        model.shape(*r).curve_type(),
                        Ok(occt::CurveType::BSpline | occt::CurveType::Bezier)
                    ) =>
                    {
                        let data = model.shape(*r).edge_bspline(1e-6, false, false).map_err(occt_error)?;
                        // A sketch's conic (one rational quadratic span) isn't a B-spline to std in Onshape: only
                        // the map (batch 070 c003).
                        if data.rational && data.degree == 2 && data.poles.len() == 3 {
                            return Ok(map(vec![("curveType", Value::string("OTHER"))]));
                        }
                        crate::bspline::bspline_value(tags, &data)
                    }
                    // The rest: std only promises a map.
                    _ => Ok(map(vec![("curveType", Value::string("OTHER"))])),
                }
            },
            &tags,
        )
    });
    m.insert("evSurfaceDefinition", |interp, args| {
        let tags = tags(interp)?;
        let as_other = definition(&args, "evSurfaceDefinition")
            .ok()
            .and_then(|d| values::opt_bool(&d, "returnBSplinesAsOther").ok().flatten())
            .unwrap_or(false);
        measure(
            args,
            "evSurfaceDefinition",
            "face",
            |tags, model, found| {
                let r = found.first().ok_or_else(|| regen_error("CANNOT_RESOLVE_ENTITIES"))?;
                if r.kind != EntityType::Face {
                    return Err(regen_error("INVALID_INPUT"));
                }
                let mut s = model.shape(*r).surface_definition().map_err(occt_error)?;
                // Cylinders and cones made by an extrude or revolve carry the coordinate system Onshape gives them.
                if let (1 | 2, Some(frame)) = (s.kind, model.info(*r).surface_frame) {
                    s.frame = frame;
                }
                let cs = || tags.coord_system(s.frame);
                // Every definition carries its `SurfaceType`, as Onshape's do (std tells extruded surfaces by it).
                let surface_type =
                    fs_interp::types::find("SurfaceType").ok_or_else(|| error("std type SurfaceType isn't loaded"))?;
                let kind = |name: &str| ("surfaceType", tags.enum_value(surface_type, name));
                match s.kind {
                    0 => {
                        let plane = model.shape(*r).face_plane().map_err(occt_error)?;
                        let plane = occt::Frame { x: values::canonical_x(plane.normal), ..plane };
                        tags.tagged(
                            map(vec![
                                ("origin", tags.length_vector(plane.origin)),
                                ("normal", tags.unit_vector(plane.normal)),
                                ("x", tags.unit_vector(plane.x)),
                                kind("PLANE"),
                            ]),
                            "Plane",
                        )
                    }
                    1 => tags.tagged(
                        map(vec![("coordSystem", cs()?), ("radius", tags.length(s.radius1)), kind("CYLINDER")]),
                        "Cylinder",
                    ),
                    2 => tags.tagged(
                        map(vec![
                            ("coordSystem", cs()?),
                            ("halfAngle", tags.with_units(s.radius1, "radian", 1.0)),
                            kind("CONE"),
                        ]),
                        "Cone",
                    ),
                    3 => tags.tagged(
                        map(vec![("coordSystem", cs()?), ("radius", tags.length(s.radius1)), kind("SPHERE")]),
                        "Sphere",
                    ),
                    4 => tags.tagged(
                        map(vec![
                            ("coordSystem", cs()?),
                            ("radius", tags.length(s.radius1)),
                            ("minorRadius", tags.length(s.radius2)),
                            kind("TORUS"),
                        ]),
                        "Torus",
                    ),
                    _ if !as_other
                        && matches!(
                            model.shape(*r).surface_type(),
                            Ok(occt::SurfaceType::BSpline | occt::SurfaceType::Bezier)
                        ) =>
                    {
                        let data = model.shape(*r).face_bspline().map_err(occt_error)?;
                        crate::bspline::bspline_surface_value(tags, &data)
                    }
                    // A surface swept straight from a curve (a sheet extruded from a spline, batch 039).
                    _ if model.shape(*r).surface_type().ok() == Some(occt::SurfaceType::Extrusion) => {
                        Ok(map(vec![kind("EXTRUDED")]))
                    }
                    _ => Ok(map(vec![kind("OTHER")])),
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
    let maximum = values::opt_bool(definition, "maximum")?.unwrap_or(false);
    let mut best: Option<(f64, usize, usize, occt::Distance)> = None;
    for (i, a) in shapes0.iter().enumerate() {
        for (j, b) in shapes1.iter().enumerate() {
            let d = if maximum { farthest(a, b)? } else { a.distance(b).map_err(occt_error)? };
            let better = match &best {
                None => true,
                Some((m, ..)) if maximum => d.distance > *m + 1e-12,
                Some((m, ..)) => d.distance < *m - 1e-12,
            };
            if better {
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

/// The greatest distance from a point to a vertex or edge (`evDistance` with `maximum`, an end cap's circle,
/// batch 019): over the edge's ends and samples along it, refined round the best.
fn farthest(a: &occt::Shape, b: &occt::Shape) -> R<occt::Distance> {
    let point_of = |s: &occt::Shape| s.vertex_point().ok();
    let (p, other, flipped) = match (point_of(a), point_of(b)) {
        (Some(p), _) => (p, b, false),
        (None, Some(q)) => (q, a, true),
        _ => return Err(error("@evDistance: the maximum distance needs a point on one side")),
    };
    let at = |t: f64| -> Option<Vec3> {
        match other.kind() {
            Some(occt::ShapeKind::Vertex) => other.vertex_point().ok(),
            _ => other.edge_tangent(t, false).ok().map(|a| a.origin),
        }
    };
    let dist = |q: Vec3| values::norm(values::sub(q, p));
    let n = 256;
    let mut best = (f64::MIN, 0.0, p);
    for k in 0..=n {
        let t = k as f64 / n as f64;
        if let Some(q) = at(t)
            && dist(q) > best.0
        {
            best = (dist(q), t, q);
        }
    }
    // Golden-section search round the best sample.
    let (mut lo, mut hi) = ((best.1 - 1.0 / n as f64).max(0.0), (best.1 + 1.0 / n as f64).min(1.0));
    for _ in 0..60 {
        let (m1, m2) = (lo + (hi - lo) * 0.382, lo + (hi - lo) * 0.618);
        let (d1, d2) = (at(m1).map_or(f64::MIN, dist), at(m2).map_or(f64::MIN, dist));
        if d1 < d2 { lo = m1 } else { hi = m2 }
    }
    if let Some(q) = at((lo + hi) / 2.0)
        && dist(q) > best.0
    {
        best = (dist(q), (lo + hi) / 2.0, q);
    }
    let (a_point, b_point) = if flipped { (best.2, p) } else { (p, best.2) };
    Ok(occt::Distance { distance: best.0, a: a_point, b: b_point })
}

/// A B-spline or Bezier edge that is straight: its start and direction.
fn straight_spline(edge: &occt::Shape) -> Option<(Vec3, Vec3)> {
    if !matches!(edge.curve_type().ok()?, occt::CurveType::BSpline | occt::CurveType::Bezier) {
        return None;
    }
    let start = edge.edge_tangent(0.0, false).ok()?.origin;
    let end = edge.edge_tangent(1.0, false).ok()?.origin;
    let chord = values::sub(end, start);
    let length = values::norm(chord);
    let direction = values::normalize(chord)?;
    let straight = [0.2, 0.4, 0.5, 0.6, 0.8].iter().all(|&t| {
        edge.edge_tangent(t, false).is_ok_and(|a| {
            let off = values::sub(a.origin, start);
            let along = values::scale(direction, values::dot(off, direction));
            values::norm(values::sub(off, along)) <= 1e-9 * length.max(1.0)
        })
    });
    straight.then_some((start, direction))
}

/// One body's offset face pairs: see `evOffsetDetection`.
fn offset_group(model: &Model, b: usize) -> R<Option<Value>> {
    use crate::values::{dot, scale, sub};
    let body = &model.bodies[b];
    let planes: Vec<(usize, occt::Frame)> =
        body.faces.iter().enumerate().filter_map(|(i, f)| f.shape.face_plane().ok().map(|p| (i, p))).collect();
    // Each face's nearest opposite face behind it that it overlaps.
    let partner = |i: usize, p: &occt::Frame| -> Option<(usize, f64)> {
        let mut best: Option<(usize, f64)> = None;
        for (j, q) in &planes {
            if *j == i || dot(p.normal, q.normal) > -1.0 + 1e-9 {
                continue;
            }
            let d = -dot(sub(q.origin, p.origin), p.normal);
            if d <= 1e-9 {
                continue;
            }
            // A point inside one face, moved across, lands on the other.
            let over = |a: usize, onto: usize, along: occt::Vec3, by: f64| {
                body.faces[a].shape.face_inner_point().ok().is_some_and(|c| {
                    occt::vertex(sub(c, scale(along, by)))
                        .and_then(|v| v.distance(&body.faces[onto].shape))
                        .is_ok_and(|d| d.distance < 1e-9)
                })
            };
            if !(over(i, *j, p.normal, d) || over(*j, i, q.normal, d)) {
                continue;
            }
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((*j, d));
            }
        }
        best
    };
    let mut pairs: Vec<(usize, usize, f64)> = Vec::new();
    for (i, p) in &planes {
        let Some((j, d)) = partner(*i, p) else { continue };
        let mutual =
            planes.iter().find(|(k, _)| *k == j).and_then(|(_, q)| partner(j, q)).is_some_and(|(k, _)| k == *i);
        if mutual && !pairs.iter().any(|(a, c, _)| (*a, *c) == (j, *i)) {
            pairs.push((*i, j, d));
        }
    }
    // Coaxial cylinders a thickness apart, each the nearest to the other that it overlaps (a rounded bend's two
    // sides, a rolled wall's, batch 042).
    let cylinders: Vec<(usize, occt::Vec3, occt::Vec3, f64)> = body
        .faces
        .iter()
        .enumerate()
        .filter_map(|(i, f)| {
            let s = f.shape.surface_definition().ok()?;
            (s.kind == 1).then_some((i, s.frame.origin, s.frame.normal, s.radius1))
        })
        .collect();
    let coaxial = |a: &(usize, occt::Vec3, occt::Vec3, f64), c: &(usize, occt::Vec3, occt::Vec3, f64)| {
        let across = sub(c.1, a.1);
        crate::values::norm(crate::values::cross(a.2, c.2)) < 1e-9
            && crate::values::norm(crate::values::cross(across, a.2)) < 1e-9
    };
    // A point inside one cylinder face, moved radially to the other's radius, lands on it.
    let overlaps = |a: &(usize, occt::Vec3, occt::Vec3, f64), c: &(usize, occt::Vec3, occt::Vec3, f64)| {
        body.faces[a.0].shape.face_inner_point().ok().is_some_and(|p| {
            let rel = sub(p, a.1);
            let radial = sub(rel, scale(a.2, dot(rel, a.2)));
            let Some(out) = crate::values::normalize(radial) else { return false };
            let q = crate::values::add(p, scale(out, c.3 - a.3));
            occt::vertex(q).and_then(|v| v.distance(&body.faces[c.0].shape)).is_ok_and(|d| d.distance < 1e-9)
        })
    };
    let nearest = |a: &(usize, occt::Vec3, occt::Vec3, f64)| {
        cylinders
            .iter()
            .filter(|c| c.0 != a.0 && (c.3 - a.3).abs() > 1e-9 && coaxial(a, c) && overlaps(a, c))
            .min_by(|x, y| (x.3 - a.3).abs().total_cmp(&(y.3 - a.3).abs()))
    };
    for a in &cylinders {
        let Some(c) = nearest(a) else { continue };
        let mutual = nearest(c).is_some_and(|back| back.0 == a.0);
        if mutual && !pairs.iter().any(|(x, y, _)| (*x, *y) == (c.0, a.0)) {
            pairs.push((a.0, c.0, (c.3 - a.3).abs()));
        }
    }
    // The commonest thickness, the thinnest of those as common (an L 2 mm thick at its base and 3 mm at its
    // upright is recognized at 2 mm, batch 042).
    let count = |t: f64| pairs.iter().filter(|q| (q.2 - t).abs() < 1e-7).count();
    let Some(thickness) = pairs.iter().map(|p| p.2).min_by(|a, b| count(*b).cmp(&count(*a)).then(a.total_cmp(b)))
    else {
        return Ok(None);
    };
    let pairs: Vec<(usize, usize)> =
        pairs.into_iter().filter(|p| (p.2 - thickness).abs() < 1e-7).map(|p| (p.0, p.1)).collect();
    // Sides: from the first pair, a neighbour's side0 is its face sharing an edge with a side0 face.
    let touches = |a: usize, c: usize| {
        let theirs = body.faces[c].shape.visible_sub_shapes(occt::ShapeKind::Edge);
        body.faces[a]
            .shape
            .visible_sub_shapes(occt::ShapeKind::Edge)
            .iter()
            .any(|e| theirs.iter().any(|f| f.is_same(e)))
    };
    let mut sides: Vec<Option<(usize, usize)>> = vec![None; pairs.len()];
    // The first pair's side0 is its larger face (the full base of an L whose upright sits on it, batch 042).
    let area = |i: usize| body.faces[i].shape.area().unwrap_or(0.0);
    let (a0, c0) = pairs[0];
    sides[0] = Some(if area(c0) > area(a0) + 1e-12 { (c0, a0) } else { (a0, c0) });
    let mut changed = true;
    while changed {
        changed = false;
        for k in 0..pairs.len() {
            if sides[k].is_some() {
                continue;
            }
            let (a, c) = pairs[k];
            let mut decided = None;
            for s in sides.iter().flatten() {
                if touches(a, s.0) || touches(c, s.1) {
                    decided = Some((a, c));
                } else if touches(c, s.0) || touches(a, s.1) {
                    decided = Some((c, a));
                }
                if decided.is_some() {
                    break;
                }
            }
            if decided.is_some() {
                sides[k] = decided;
                changed = true;
            }
        }
    }
    let tid = |i: usize| Value::string(crate::query::tid_string(body.faces[i].tid));
    let (side0, side1): (Vec<Value>, Vec<Value>) = sides.iter().flatten().map(|&(a, c)| (tid(a), tid(c))).unzip();
    Ok(Some(crate::values::map(vec![
        ("side0", Value::array(side0)),
        ("side1", Value::array(side1)),
        ("offsetLow", Value::number(thickness)),
        ("offsetHigh", Value::number(thickness)),
    ])))
}
