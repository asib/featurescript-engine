//! Mate connectors: `@opMateConnector` makes one (a point body with a coordinate system), `@evMateConnector` reads
//! it back, and `@evMateConnectorCoordSystem` works out the coordinate system std's mate connector feature asks for
//! from the entity picked and how to infer from it.
//!
//! The rules, from fixture batch 005 (c024–c029): a face's connector sits at its centroid with z along its normal;
//! a circle's at its centre with z along its axis; an edge's mid point has z along the edge; a vertex's frame is the
//! world's. Whenever z comes from the geometry, x is Onshape's canonical x for that z (as for planes). Flipping the
//! primary axis turns the frame half a turn about y; the secondary axis type then makes ±x or ±y the new x; the
//! translation is along the axes so far, and the rotation turns the frame about one of them last.

use crate::model::{BodyType, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, Tags, field};
use crate::{GeomBuiltins, model_ref, occt_error, query, regen_error, tags};
use fs_interp::Value;
use fs_interp::builtins::arg;
use fs_interp::eval::{R, error};
use occt::{Frame, Vec3};

fn frame_with_canonical_x(origin: Vec3, normal: Vec3) -> Frame {
    Frame { origin, normal, x: values::canonical_x(normal) }
}

fn world(origin: Vec3) -> Frame {
    Frame { origin, normal: Vec3::new(0.0, 0.0, 1.0), x: Vec3::new(1.0, 0.0, 0.0) }
}

fn y_of(f: &Frame) -> Vec3 {
    values::cross(f.normal, f.x)
}

/// A `CoordSystem` value.
pub fn coord_system(v: &Value, what: &str) -> R<Frame> {
    Ok(Frame {
        origin: values::vec3(field(v, "origin"), what)?,
        normal: values::normalize(values::vec3(field(v, "zAxis"), what)?)
            .ok_or_else(|| error(format!("{what}: zero z")))?,
        x: values::normalize(values::vec3(field(v, "xAxis"), what)?).ok_or_else(|| error(format!("{what}: zero x")))?,
    })
}

/// The frame inferred from one entity.
fn inferred(model: &Model, definition: &Value) -> R<Frame> {
    let inference = field(definition, "entityInferenceType").as_str().unwrap_or("POINT");
    let found = query::evaluate(model, field(definition, "originQuery"))?;
    let Some(r) = found.first().copied() else {
        return match inference {
            "PART_ORIGIN" | "ORIGIN_X" | "ORIGIN_Y" | "ORIGIN_Z" => Ok(world(Vec3::new(0.0, 0.0, 0.0))),
            _ => Err(regen_error("MATECONNECTOR_ORIGIN_NOT_RESOLVED")),
        };
    };
    let shape = model.shape(r).clone();
    let failed = || regen_error("MATECONNECTOR_ORIGIN_NOT_RESOLVED");
    Ok(match (r.kind, inference) {
        (EntityType::Vertex, _) | (EntityType::Body, _) if model.body(r).body_type == BodyType::MateConnector => {
            model.body(r).frame.ok_or_else(failed)?
        }
        (EntityType::Vertex, _) => world(shape.vertex_point().map_err(occt_error)?),
        (EntityType::Face, "CENTROID") => {
            let centroid = shape.centroid().map_err(occt_error)?;
            match shape.face_plane() {
                Ok(plane) => frame_with_canonical_x(centroid, plane.normal),
                Err(_) => {
                    let (u, v) = shape.face_fraction(centroid).map_err(occt_error)?;
                    let tangent = shape.face_tangent_plane(u, v).map_err(occt_error)?;
                    frame_with_canonical_x(centroid, tangent.normal)
                }
            }
        }
        // A cone's tip, z along the cone's z (batch 090 c008).
        (EntityType::Face, "POINT") if shape.surface_definition().is_ok_and(|s| s.kind == 2) => {
            let apex = shape.surface_definition().map_err(occt_error)?.frame.origin;
            let z =
                model.info(r).surface_frame.map(|f| f.normal).unwrap_or(shape.axis().map_err(|_| failed())?.direction);
            frame_with_canonical_x(apex, z)
        }
        (EntityType::Face, "CENTER" | "MID_AXIS_POINT" | "TOP_AXIS_POINT" | "BOTTOM_AXIS_POINT") => {
            // A point on the face's axis: its middle, top or bottom over the face's extent along the axis, which
            // runs along Onshape's z for the face (a revolved cone's down its axis, batch 090 c003, c004).
            let axis = shape.axis().map_err(|_| failed())?;
            let axis = match model.info(r).surface_frame {
                Some(f) if values::norm(values::cross(f.normal, axis.direction)) < 1e-9 => {
                    occt::Axis { origin: axis.origin, direction: f.normal }
                }
                _ => axis,
            };
            let frame = frame_with_canonical_x(axis.origin, axis.direction);
            let extent = shape.bounding_box_in(frame, true).map_err(occt_error)?;
            let along = match inference {
                "TOP_AXIS_POINT" => extent.max.z,
                "BOTTOM_AXIS_POINT" => extent.min.z,
                _ => (extent.min.z + extent.max.z) / 2.0,
            };
            frame_with_canonical_x(values::add(axis.origin, values::scale(axis.direction, along)), axis.direction)
        }
        (EntityType::Edge, "CENTER") => {
            let axis = shape.axis().map_err(|_| failed())?;
            frame_with_canonical_x(axis.origin, axis.direction)
        }
        (EntityType::Edge, "MID_POINT" | "POINT") => {
            let tangent = shape.edge_tangent(0.5, true).map_err(occt_error)?;
            frame_with_canonical_x(tangent.origin, tangent.direction)
        }
        // A loop's centre and a virtual sharp aren't for FeatureScript (batch 090 c005-c007).
        (_, "LOOP_CENTER" | "VIRTUAL_SHARP") => return Err(regen_error("MATECONNECTOR_QUERY_CSYS_FAILED")),
        (kind, inference) => {
            return Err(error(format!(
                "@evMateConnectorCoordSystem: inferring {inference} from a {} isn't supported yet",
                kind.name()
            )));
        }
    })
}

/// The direction an entity gives an axis: a line's, a plane's normal, a cylinder's or cone's axis.
fn axis_direction(model: &Model, query: &Value) -> R<Vec3> {
    let found = query::evaluate(model, query)?;
    let r = found.first().copied().ok_or_else(|| regen_error("MATECONNECTOR_AXIS_NOT_RESOLVED"))?;
    let shape = model.shape(r);
    let direction = match r.kind {
        EntityType::Face => match shape.face_plane() {
            Ok(plane) => plane.normal,
            Err(_) => shape.axis().map_err(occt_error)?.direction,
        },
        _ => shape.axis().map_err(occt_error)?.direction,
    };
    values::normalize(direction).ok_or_else(|| regen_error("MATECONNECTOR_AXIS_NOT_RESOLVED"))
}

fn coord_system_for(model: &Model, definition: &Value) -> R<Frame> {
    // A given normal is for connectors placed on meshes; on other entities Onshape ignores it (batch 020).
    let mut f = inferred(model, definition)?;
    // Between entities: halfway from the inferred origin to the other face, turned as the inferred frame is
    // (batch 018).
    if field(definition, "originType").as_str() == Some("BETWEEN_ENTITIES") {
        let found = query::evaluate(model, field(definition, "originAdditionalQuery"))?;
        let r = found.first().copied().ok_or_else(|| regen_error("MATECONNECTOR_ORIGIN_NOT_RESOLVED"))?;
        let near = occt::vertex(f.origin).and_then(|v| v.distance(model.shape(r))).map_err(occt_error)?.b;
        f.origin = values::scale(values::add(f.origin, near), 0.5);
    }
    // Realigned: z along the primary axis entity, x along the secondary one, square to z (batch 018).
    if values::opt_bool(definition, "realign")?.unwrap_or(false) {
        let z = axis_direction(model, field(definition, "primaryAxisQuery"))?;
        let s = axis_direction(model, field(definition, "secondaryAxisQuery"))?;
        f.normal = z;
        f.x = values::normalize(values::sub(s, values::scale(z, values::dot(s, z))))
            .ok_or_else(|| regen_error("MATECONNECTOR_AXES_PARALLEL"))?;
    }
    if values::opt_bool(definition, "flipPrimary")?.unwrap_or(false) {
        f.normal = values::scale(f.normal, -1.0);
        f.x = values::scale(f.x, -1.0);
    }
    let y = y_of(&f);
    f.x = match field(definition, "secondaryAxisType").as_str() {
        None | Some("PLUS_X") => f.x,
        Some("PLUS_Y") => y,
        Some("MINUS_X") => values::scale(f.x, -1.0),
        Some("MINUS_Y") => values::scale(y, -1.0),
        Some(other) => return Err(error(format!("unknown secondary axis type {other}"))),
    };
    if values::opt_bool(definition, "transform")?.unwrap_or(false) {
        let length = |key: &str| values::length(field(definition, key), key);
        let (tx, ty, tz) = (length("translationX")?, length("translationY")?, length("translationZ")?);
        let y = y_of(&f);
        f.origin = values::add(
            f.origin,
            values::add(values::scale(f.x, tx), values::add(values::scale(y, ty), values::scale(f.normal, tz))),
        );
        let angle = values::angle(field(definition, "rotation"), "rotation")?;
        let axis = match field(definition, "rotationType").as_str() {
            Some("ABOUT_X") => f.x,
            Some("ABOUT_Y") => y,
            _ => f.normal,
        };
        let r = crate::ops::rotation(occt::Axis { origin: Vec3::new(0.0, 0.0, 0.0), direction: axis }, angle).m;
        let turn = |v: Vec3| {
            Vec3::new(
                r[0] * v.x + r[1] * v.y + r[2] * v.z,
                r[4] * v.x + r[5] * v.y + r[6] * v.z,
                r[8] * v.x + r[9] * v.y + r[10] * v.z,
            )
        };
        f.x = turn(f.x);
        f.normal = turn(f.normal);
    }
    let remaining = field(definition, "remainingTransform");
    if !remaining.is_undefined() {
        f = crate::ops::transform_frame(&f, &crate::ops::transform(remaining)?);
    }
    Ok(f)
}

/// The first mate connector a query finds.
fn connector(model: &Model, query: &Value) -> R<Frame> {
    let found = query::evaluate(model, query)?;
    found
        .iter()
        .find_map(|r| {
            let body = model.body(*r);
            (body.body_type == BodyType::MateConnector).then_some(body.frame).flatten()
        })
        .ok_or_else(|| regen_error("CANNOT_RESOLVE_ENTITIES"))
}

pub fn evaluate_connector(tags: &Tags, model: &Model, query: &Value) -> R<Value> {
    tags.coord_system(connector(model, query)?)
}

pub fn make_connector(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let frame = coord_system(field(definition, "coordSystem"), "opMateConnector coordSystem")?;
    let info = Info::new(id);
    let shape = occt::vertex(frame.origin).map_err(occt_error)?;
    let owner = match field(definition, "owner") {
        v if v.is_undefined() => None,
        v => crate::query::evaluate(model, v)?.first().map(|r| model.bodies[r.body].tid),
    };
    let new = NewBody { frame: Some(frame), owner, ..NewBody::new(shape, BodyType::MateConnector, info.clone()) };
    model.add_body(new, &Origins::default(), &info, &|_| None);
    Ok(Value::UNDEFINED)
}

pub fn register(m: &mut GeomBuiltins) {
    m.insert("evMateConnector", |interp, args| {
        let tags = tags(interp)?;
        let definition = arg(&args, 1, "evMateConnector")?.clone();
        model_ref(&args, "evMateConnector", |model| {
            evaluate_connector(&tags, model, field(&definition, "mateConnector"))
        })
    });
    m.insert("evMateConnectorCoordSystem", |interp, args| {
        let tags = tags(interp)?;
        let definition = arg(&args, 1, "evMateConnectorCoordSystem")?.clone();
        model_ref(&args, "evMateConnectorCoordSystem", |model| tags.coord_system(coord_system_for(model, &definition)?))
    });
}
