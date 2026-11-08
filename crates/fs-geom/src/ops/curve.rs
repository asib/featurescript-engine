//! Curve operations: `@opHelix`, `@opFitSpline`, `@opCreateBSplineCurve`, `@opEditCurve` and `@opPolyline`.

use crate::model::{BodyType, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::splines::{self, Ends};
use crate::values::{self, field};
use crate::{occt_error, query, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};

fn add_wire(model: &mut Model, id: FeatureId, edge: occt::Shape) {
    let info = Info::new(id);
    model.add_body(NewBody::new(edge, BodyType::Wire, info.clone()), &Origins::default(), &info, &|_| None);
}

pub fn helix(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let direction = values::vec3(field(definition, "direction"), "opHelix direction")?;
    let origin = values::vec3(field(definition, "axisStart"), "opHelix axisStart")?;
    let start = values::vec3(field(definition, "startPoint"), "opHelix startPoint")?;
    let interval = values::numbers(field(definition, "interval"), "opHelix interval")?;
    let [t0, t1] = interval[..] else { return Err(error("@opHelix: interval must have two numbers")) };
    let clockwise = values::opt_bool(definition, "clockwise")?.unwrap_or(false);
    let pitch = values::length(field(definition, "helicalPitch"), "opHelix helicalPitch")?;
    let spiral = match field(definition, "spiralPitch") {
        v if v.is_undefined() => 0.0,
        v => values::length(v, "opHelix spiralPitch")?,
    };
    // `clockwise: false` turns left-handed about `direction` (batch 004, c043; whole turns can't tell): the
    // opposite of occt's flag.
    let edge = occt::helix_edge(origin, direction, start, t0, t1, !clockwise, pitch, spiral)
        .map_err(|_| regen_error("HELIX_FAILED"))?;
    add_wire(model, id, edge);
    Ok(Value::UNDEFINED)
}

pub fn fit_spline(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    if values::opt_bool(definition, "hasTargetLength")?.unwrap_or(false) {
        return Err(error("@opFitSpline: target lengths aren't supported yet"));
    }
    let items = field(definition, "points").as_array().ok_or_else(|| error("@opFitSpline: points must be an array"))?;
    let mut points: Vec<occt::Vec3> = items.iter().map(|p| values::vec3(p, "opFitSpline point")).collect::<R<_>>()?;
    let closed = points.len() > 2 && values::norm(values::sub(points[0], *points.last().expect("points"))) < 1e-12;
    if closed {
        points.pop();
    }
    let derivative = |key: &str| -> R<Option<occt::Vec3>> {
        match field(definition, key) {
            v if v.is_undefined() => Ok(None),
            v => values::vec3(v, key).map(Some),
        }
    };
    // A second derivative at an end (matching a curve's curvature there, batch 027).
    let (second_start, second_end) = (derivative("start2ndDerivative")?, derivative("end2ndDerivative")?);
    // Derivatives at inner points, by index (a routing curve's, batch 035; std leaves the ends' undefined).
    let mut inner: Vec<(usize, occt::Vec3)> = Vec::new();
    if let Some(map) = field(definition, "derivatives").as_map() {
        for (key, value) in map.iter() {
            if value.is_undefined() {
                continue;
            }
            let index = key.as_number().ok_or_else(|| error("@opFitSpline: derivatives must be keyed by point index"))?;
            inner.push((index as usize, values::vec3(value, "opFitSpline derivative")?));
        }
    }
    if !inner.is_empty() {
        if closed || second_start.is_some() || second_end.is_some() {
            return Err(error("@opFitSpline: inner derivatives of closed splines or with second derivatives aren't supported yet"));
        }
        let edge = splines::spline_with_point_derivatives(&points, derivative("startDerivative")?, derivative("endDerivative")?, &inner)
            .map_err(|_| regen_error("FIT_SPLINE_FAILED"))?;
        add_wire(model, id, edge);
        return Ok(Value::UNDEFINED);
    }
    if !closed && (second_start.is_some() || second_end.is_some()) {
        let conditions = |first: Option<occt::Vec3>, second: Option<occt::Vec3>| -> R<Vec<occt::Vec3>> {
            match (first, second) {
                (Some(d1), Some(d2)) => Ok(vec![d1, d2]),
                (Some(d1), None) => Ok(vec![d1]),
                (None, None) => Ok(Vec::new()),
                (None, Some(_)) => Err(error("@opFitSpline: a second derivative needs a first")),
            }
        };
        let start = conditions(derivative("startDerivative")?, second_start)?;
        let end = conditions(derivative("endDerivative")?, second_end)?;
        let edge = splines::spline_with_end_derivatives(&points, &start, &end).map_err(|_| regen_error("FIT_SPLINE_FAILED"))?;
        add_wire(model, id, edge);
        return Ok(Value::UNDEFINED);
    }
    let ends = match (closed, derivative("startDerivative")?, derivative("endDerivative")?) {
        (true, _, _) => Ends::Periodic,
        (false, Some(a), Some(b)) => Ends::Clamped(a, b),
        (false, None, None) => Ends::Natural,
        (false, Some(a), None) => Ends::StartClamped(a),
        (false, None, Some(b)) => Ends::EndClamped(b),
    };
    let parameters = match field(definition, "parameters") {
        v if v.is_undefined() => None,
        v => Some(values::numbers(v, "opFitSpline parameters")?),
    };
    // Through two points with nothing more to fit, a spline is straight: a line, which then merges with lines it
    // continues (a rib's profile extended to the part, batch 019; a jog's second bend line).
    let edge = if points.len() == 2 && matches!(ends, Ends::Natural) {
        occt::line_edge(points[0], points[1]).map_err(|_| regen_error("FIT_SPLINE_FAILED"))?
    } else {
        splines::spline(&points, ends, parameters).map_err(|_| regen_error("FIT_SPLINE_FAILED"))?
    };
    add_wire(model, id, edge);
    Ok(Value::UNDEFINED)
}

/// `@opCreateBSplineCurve`: a wire body on a B-spline given as std's `BSplineCurve`.
pub fn bspline_curve(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let curve = field(definition, "bSplineCurve");
    let degree = field(curve, "degree").as_number().ok_or_else(|| error("@opCreateBSplineCurve: no degree"))? as i32;
    if field(curve, "dimension").as_number() != Some(3.0) {
        return Err(error("@opCreateBSplineCurve: only 3D curves are supported"));
    }
    let poles: Vec<occt::Vec3> = field(curve, "controlPoints")
        .as_array()
        .map(|a| a.iter().map(|p| values::vec3(p, "opCreateBSplineCurve control point")).collect::<R<_>>())
        .transpose()?
        .unwrap_or_default();
    let weights = if field(curve, "isRational").as_bool() == Some(true) {
        values::numbers(field(curve, "weights"), "opCreateBSplineCurve weights")?
    } else {
        Vec::new()
    };
    let knots = values::numbers(field(curve, "knots"), "opCreateBSplineCurve knots")?;
    let periodic = field(curve, "isPeriodic").as_bool() == Some(true);
    let edge = occt::bspline_edge(&poles, &weights, &knots, degree, periodic).map_err(crate::occt_error)?;
    add_wire(model, id, edge);
    Ok(Value::UNDEFINED)
}

/// `@opEditCurve`: a wire body takes the curve of another edge. It keeps its identity; its edge and end vertices
/// are the old ones, changed (one edge, however many it had).
pub fn edit_curve(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let mut bodies: Vec<usize> = query::evaluate(model, field(definition, "wire"))?.iter().map(|r| r.body).collect();
    bodies.dedup();
    let [b] = bodies[..] else { return Err(regen_error("EDIT_CURVE_SELECT_WIRE")) };
    if model.bodies[b].body_type != BodyType::Wire {
        return Err(regen_error("EDIT_CURVE_SELECT_WIRE"));
    }
    let edge = query::evaluate(model, field(definition, "edge"))?
        .into_iter()
        .find(|r| r.kind == EntityType::Edge)
        .ok_or_else(|| regen_error("CANNOT_RESOLVE_ENTITIES"))?;
    // A fresh edge on the same curve, so the two bodies share no topology.
    let data = model.shape(edge).edge_bspline(1e-7, false, false).map_err(occt_error)?;
    let new_edge =
        occt::bspline_edge(&data.poles, &data.weights, &data.knots, data.degree, data.periodic).map_err(occt_error)?;
    let wire = occt::connect_edges(std::slice::from_ref(&new_edge))
        .map_err(occt_error)?
        .into_iter()
        .next()
        .ok_or_else(|| error("@opEditCurve: no wire"))?;
    let old = model.bodies[b].clone();
    let mut origins = Origins::default();
    if let (Some(old_edge), Some(image)) = (old.edges.first(), wire.sub_shapes(occt::ShapeKind::Edge).first()) {
        origins.inherit(image, old_edge);
    }
    // The old ends, in order along the old wire, become the new ones.
    let ends = |shape: &occt::Shape| -> Vec<occt::Shape> {
        let vertices = shape.sub_shapes(occt::ShapeKind::Vertex);
        let first = shape.edge_tangent(0.0, false).ok().map(|a| a.origin);
        let mut v = vertices;
        if let Some(first) = first {
            v.sort_by(|x, y| {
                let d =
                    |s: &occt::Shape| s.vertex_point().map(|p| values::norm(values::sub(p, first))).unwrap_or(f64::MAX);
                d(x).total_cmp(&d(y))
            });
        }
        v
    };
    let new_ends = ends(&new_edge);
    let old_ends: Vec<_> = match old.edges[..] {
        [ref only] => ends(&only.shape),
        _ => Vec::new(),
    };
    for (image, old_vertex) in new_ends.iter().zip(&old_ends) {
        if let Some(entity) = old.vertices.iter().find(|v| v.shape.is_same(old_vertex)) {
            let image = wire.sub_shapes(occt::ShapeKind::Vertex).into_iter().find(|w| w.is_same(image));
            if let Some(image) = image {
                origins.inherit(&image, entity);
            }
        }
    }
    let new = NewBody { tid: Some(old.tid), body_type: BodyType::Wire, ..NewBody::replacing(&old, wire) };
    model.remove_bodies(&[b]);
    model.add_body(new, &origins, &Info::new(id), &|_| None);
    Ok(Value::UNDEFINED)
}

/// `@opPolyline`: a wire body of lines through the points, with a tangent arc of the given radius at each bend
/// that has one (`bendRadii[i]` is the bend at point i + 1). It's closed if the first point is the last.
pub fn polyline(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    use values::{add, cross, dot, norm, normalize, scale, sub};
    let mut points: Vec<occt::Vec3> = field(definition, "points")
        .as_array()
        .ok_or_else(|| error("@opPolyline: points must be an array"))?
        .iter()
        .map(|p| values::vec3(p, "opPolyline point"))
        .collect::<R<_>>()?;
    points.dedup_by(|a, b| norm(sub(*a, *b)) < 1e-9);
    let closed = points.len() > 2 && norm(sub(points[0], points[points.len() - 1])) < 1e-9;
    if closed {
        points.pop();
    }
    let n = points.len();
    if n < 2 {
        return Err(regen_error("ROUTING_CURVE_AT_LEAST_TWO_DISTINCT_POINTS"));
    }
    let radii: Vec<f64> = match field(definition, "bendRadii") {
        v if v.is_undefined() => Vec::new(),
        v => v
            .as_array()
            .ok_or_else(|| error("@opPolyline: bendRadii must be an array"))?
            .iter()
            .map(|r| values::length(r, "opPolyline bend radius"))
            .collect::<R<_>>()?,
    };
    // Each corner's radius: corner i is at point i; an open polyline has none at its ends.
    let corner_radius = |i: usize| -> f64 {
        if closed {
            // bendRadii[k] is the bend at point k + 1, wrapping round to the start.
            radii.get((i + n - 1) % n).copied().unwrap_or(0.0)
        } else if i == 0 || i == n - 1 {
            0.0
        } else {
            radii.get(i - 1).copied().unwrap_or(0.0)
        }
    };
    // Where each corner's lines end and its arc (if any), as (start, end, arc through).
    let mut corners: Vec<(occt::Vec3, occt::Vec3, Option<occt::Vec3>)> = Vec::new();
    for i in 0..n {
        let p = points[i];
        let r = corner_radius(i);
        let neighbours =
            if closed || (i > 0 && i < n - 1) { Some((points[(i + n - 1) % n], points[(i + 1) % n])) } else { None };
        let (Some((before, after)), true) = (neighbours, r > 0.0) else {
            corners.push((p, p, None));
            continue;
        };
        let (Some(d_in), Some(d_out)) = (normalize(sub(p, before)), normalize(sub(after, p))) else {
            corners.push((p, p, None));
            continue;
        };
        let turn = dot(d_in, d_out).clamp(-1.0, 1.0).acos();
        if turn < 1e-9 || norm(cross(d_in, d_out)) < 1e-12 {
            corners.push((p, p, None));
            continue;
        }
        let t = r * (turn / 2.0).tan();
        let t1 = sub(p, scale(d_in, t));
        let t2 = add(p, scale(d_out, t));
        let bisector = normalize(sub(d_out, d_in)).ok_or_else(|| regen_error("POLYLINE_INVALID_BEND"))?;
        let centre = add(p, scale(bisector, r / (turn / 2.0).cos()));
        let mid = add(centre, scale(normalize(sub(p, centre)).expect("the corner is off the centre"), r));
        corners.push((t1, t2, Some(mid)));
    }
    let mut edges = Vec::new();
    let segments = if closed { n } else { n - 1 };
    for i in 0..segments {
        let j = (i + 1) % n;
        let (start, end) = (corners[i].1, corners[j].0);
        let along = sub(points[j], points[i]);
        // The arcs at both ends must leave some of the line between them.
        if dot(sub(end, start), along) < -1e-9 {
            return Err(regen_error("POLYLINE_INVALID_BEND"));
        }
        if norm(sub(end, start)) > 1e-9 {
            edges.push(occt::line_edge(start, end).map_err(occt_error)?);
        }
        if let (start, end, Some(mid)) = corners[j]
            && (closed || j < n - 1)
        {
            edges.push(occt::arc_through(start, mid, end).map_err(occt_error)?);
        }
    }
    let wire = occt::connect_edges(&edges)
        .map_err(occt_error)?
        .into_iter()
        .next()
        .ok_or_else(|| error("@opPolyline: no wire"))?;
    add_wire(model, id, wire);
    Ok(Value::UNDEFINED)
}
