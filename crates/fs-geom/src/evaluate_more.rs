//! Evaluations of std's API that no std feature calls (batch 100): approximate mass properties, points' deviation
//! from entities, and a B-spline curve's points and derivatives.

use crate::model::{BodyType, EntityType, Model};
use crate::values::{self, Tags, field, map};
use crate::{GeomBuiltins, model_mut, occt_error, query, tags};
use fs_interp::Value;
use fs_interp::builtins::arg;
use fs_interp::eval::{R, error};
use occt::{Shape, Vec3};

pub fn register(m: &mut GeomBuiltins) {
    m.insert("evApproximateMassProperties", |_, args| {
        let definition = arg(&args, 1, "evApproximateMassProperties")?.clone();
        model_mut(&args, "evApproximateMassProperties", |model| mass_properties(model, &definition))
    });
    m.insert("evPointsDeviation", |interp, args| {
        let tags = tags(interp)?;
        let definition = arg(&args, 1, "evPointsDeviation")?.clone();
        model_mut(&args, "evPointsDeviation", |model| points_deviation(&tags, model, &definition))
    });
    m.insert("evaluateSpline", |_, args| evaluate_spline(arg(&args, 0, "evaluateSpline")?));
    // A mesh body's points: the engine makes no meshes (Onshape's are imported ones), so bodies and faces that
    // aren't meshes find none, as Onshape's do (batch 105).
    m.insert("evMeshPoints", |_, args| {
        let meshes = arg(&args, 1, "evMeshPoints")?.get("meshes").cloned().unwrap_or(Value::UNDEFINED);
        model_mut(&args, "evMeshPoints", |model| {
            if crate::query::evaluate(model, &meshes)?.is_empty() {
                return Err(crate::faulty("CANNOT_RESOLVE_ENTITIES", "meshes"));
            }
            Err(crate::regen_error("NO_MESH_FOUND"))
        })
    });
    m.insert("evApproximateBSplineSurface", |interp, args| {
        let tags = tags(interp)?;
        let definition = arg(&args, 1, "evApproximateBSplineSurface")?.clone();
        model_mut(&args, "evApproximateBSplineSurface", |model| approximate_surface(&tags, model, &definition))
    });
    m.insert("evFaceCurvatureDerivatives", |_, args| {
        let definition = arg(&args, 1, "evFaceCurvatureDerivatives")?.clone();
        model_mut(&args, "evFaceCurvatureDerivatives", |model| curvature_derivatives(model, &definition))
    });
}

/// A face's curvature as a 3×3 matrix at fractions of its parameter box: each principal curvature (positive where
/// it bends away from the outward normal) times its direction's outer product.
fn curvature_matrix(face: &Shape, fu: f64, fv: f64) -> R<[f64; 9]> {
    let c = face.face_curvatures(fu, fv).map_err(occt_error)?;
    let mut m = [0.0; 9];
    for (k, d) in [(c.radius1, c.frame.normal), (c.radius2, c.frame.x)] {
        for i in 0..3 {
            for j in 0..3 {
                m[3 * i + j] += k * component(d, i) * component(d, j);
            }
        }
    }
    Ok(m)
}

/// How a face's second fundamental form changes along tangent directions at parameters (per metre), worked out
/// across a small step either way along each direction.
fn curvature_derivatives(model: &Model, definition: &Value) -> R<Value> {
    let found = query::evaluate(model, field(definition, "face"))?;
    let r = found
        .iter()
        .find(|r| r.kind == EntityType::Face)
        .ok_or_else(|| crate::regen_error("CANNOT_RESOLVE_ENTITIES"))?;
    let face = model.shape(*r);
    // Onshape's parameters to OCCT's fractions, as evFaceCurvatures takes them.
    let torus = model.info(*r).surface_frame.and_then(|frame| crate::ops::frames::torus_fractions(face, frame));
    let cone = model.info(*r).surface_frame.and_then(|frame| crate::ops::frames::cone_fractions(face, frame));
    let parameters =
        field(definition, "parameters").as_array().ok_or_else(|| error("parameters must be an array"))?.clone();
    let directions =
        field(definition, "directions").as_array().ok_or_else(|| error("directions must be an array"))?.clone();
    let mut out = Vec::new();
    for (uv, direction) in parameters.iter().zip(directions.iter()) {
        let (fu, fv) = values::vec2(uv, "evFaceCurvatureDerivatives parameter")?;
        let (fu, fv) = torus.as_ref().map_or((fu, fv), |map| map(fu, fv));
        let (fu, fv) = cone.as_ref().map_or((fu, fv), |map| map(fu, fv));
        let d = values::vec3(direction, "evFaceCurvatureDerivatives direction")?;
        let plane = face.face_tangent_plane(fu, fv).map_err(occt_error)?;
        // The direction must lie in the tangent plane: one leaning out of it is refused, not projected (batch 101
        // c005).
        let length = values::norm(d);
        if length < 1e-300 || values::dot(d, plane.normal).abs() > 1e-6 * length {
            return Err(crate::regen_error("NO_TANGENT_PLANE"));
        }
        let along = values::scale(d, 1.0 / length);
        // The parameter step whose point moves along `along`: from the parameter derivatives by small differences.
        let h = 1e-5;
        let at = |u: f64, v: f64| face.face_tangent_plane(u, v).map(|p| p.origin).map_err(occt_error);
        let su = values::scale(values::sub(at(fu + h, fv)?, at(fu - h, fv)?), 0.5 / h);
        let sv = values::scale(values::sub(at(fu, fv + h)?, at(fu, fv - h)?), 0.5 / h);
        let (a, b, c) = (values::dot(su, su), values::dot(su, sv), values::dot(sv, sv));
        let (e, f) = (values::dot(su, along), values::dot(sv, along));
        let det = a * c - b * b;
        if det.abs() < 1e-300 {
            return Err(crate::regen_error("NO_TANGENT_PLANE"));
        }
        let (du, dv) = ((c * e - b * f) / det, (a * f - b * e) / det);
        let step = 1e-4 / values::norm(values::add(values::scale(su, du), values::scale(sv, dv))).max(1e-300) * 1e-2;
        let ahead = curvature_matrix(face, fu + du * step, fv + dv * step)?;
        let behind = curvature_matrix(face, fu - du * step, fv - dv * step)?;
        let moved = values::norm(values::sub(at(fu + du * step, fv + dv * step)?, at(fu - du * step, fv - dv * step)?));
        let rows: Vec<Value> = (0..3)
            .map(|i| {
                Value::array(
                    // The second fundamental form is std's minus each curvature times its direction's outer
                    // product; its derivative per metre, in plain numbers (std adds the units).
                    (0..3).map(|j| Value::number(-(ahead[3 * i + j] - behind[3 * i + j]) / moved)).collect(),
                )
            })
            .collect();
        out.push(Value::array(rows));
    }
    Ok(Value::array(out))
}

/// An entity's dimension and shape: a solid 3, a sheet or face 2, a wire or edge 1, a point or vertex 0.
fn dimension(model: &Model, r: crate::model::EntityRef) -> (i32, Shape) {
    match r.kind {
        EntityType::Body => {
            let body = &model.bodies[r.body];
            let d = match body.body_type {
                BodyType::Solid | BodyType::Composite => 3,
                BodyType::Sheet => 2,
                BodyType::Wire => 1,
                _ => 0,
            };
            (d, body.shape.clone())
        }
        EntityType::Face => (2, model.shape(r).clone()),
        EntityType::Edge => (1, model.shape(r).clone()),
        EntityType::Vertex => (0, model.shape(r).clone()),
    }
}

/// The entities of the highest dimension's measure, centroid and inertia (unit density), in metres, about the
/// centroid in world axes or about a reference frame in its axes.
fn mass_properties(model: &Model, definition: &Value) -> R<Value> {
    let found = query::evaluate(model, field(definition, "entities"))?;
    let entities: Vec<(i32, Shape)> = found.iter().map(|r| dimension(model, *r)).collect();
    let highest =
        entities.iter().map(|(d, _)| *d).max().ok_or_else(|| crate::regen_error("CANNOT_RESOLVE_ENTITIES"))?;
    let shapes: Vec<Shape> = entities.into_iter().filter(|(d, _)| *d == highest).map(|(_, s)| s).collect();
    let (measure, centroid, mut inertia) = if highest == 0 {
        let points: Vec<Vec3> =
            shapes.iter().map(|s| s.vertex_point()).collect::<Result<_, _>>().map_err(occt_error)?;
        let n = points.len() as f64;
        let c = values::scale(points.iter().fold(Vec3::new(0.0, 0.0, 0.0), |a, p| values::add(a, *p)), 1.0 / n);
        let mut m = [0.0; 9];
        for p in &points {
            add_point_inertia(&mut m, values::sub(*p, c), 1.0);
        }
        (n, c, m)
    } else {
        occt::compound(&shapes).mass_properties(highest).map_err(occt_error)?
    };
    let mut centroid = centroid;
    let frame = field(definition, "referenceFrame");
    if !frame.is_undefined() {
        let origin = values::vec3(field(frame, "origin"), "referenceFrame origin")?;
        let z = values::normalize(values::vec3(field(frame, "zAxis"), "referenceFrame zAxis")?)
            .ok_or_else(|| error("referenceFrame zAxis is zero"))?;
        let x = values::normalize(values::vec3(field(frame, "xAxis"), "referenceFrame xAxis")?)
            .ok_or_else(|| error("referenceFrame xAxis is zero"))?;
        let y = values::cross(z, x);
        // Moved to the frame's origin (the parallel axis theorem), then turned into its axes.
        add_point_inertia(&mut inertia, values::sub(centroid, origin), measure);
        let rows = [x, y, z];
        let mut turned = [0.0; 9];
        for i in 0..3 {
            for j in 0..3 {
                let mut sum = 0.0;
                for k in 0..3 {
                    for l in 0..3 {
                        sum += component(rows[i], k) * inertia[3 * k + l] * component(rows[j], l);
                    }
                }
                turned[3 * i + j] = sum;
            }
        }
        inertia = turned;
        let d = values::sub(centroid, origin);
        centroid = Vec3::new(values::dot(d, x), values::dot(d, y), values::dot(d, z));
    }
    let numbers = |xs: &[f64]| Value::array(xs.iter().map(|&x| Value::number(x)).collect());
    let key = ["count", "length", "area", "volume"][highest as usize];
    Ok(map(vec![
        ("highestDimension", Value::number(highest as f64)),
        (key, Value::number(measure)),
        ("centroid", numbers(&[centroid.x, centroid.y, centroid.z])),
        ("inertia", Value::array(inertia.chunks(3).map(numbers).collect())),
    ]))
}

fn component(v: Vec3, i: usize) -> f64 {
    [v.x, v.y, v.z][i]
}

/// Adds a point mass `mass` at `d` from the reference point to a matrix of inertia.
fn add_point_inertia(m: &mut [f64; 9], d: Vec3, mass: f64) {
    let r2 = values::dot(d, d);
    for i in 0..3 {
        for j in 0..3 {
            let delta = if i == j { r2 } else { 0.0 };
            m[3 * i + j] += mass * (delta - component(d, i) * component(d, j));
        }
    }
}

/// Each point's distance from the nearest of the entities (nothing, inside a solid), with the nearest point; or
/// only the farthest point's.
fn points_deviation(tags: &Tags, model: &Model, definition: &Value) -> R<Value> {
    let points = field(definition, "points")
        .as_array()
        .ok_or_else(|| error("@evPointsDeviation: points must be an array"))?
        .iter()
        .map(|p| values::vec3(p, "evPointsDeviation point"))
        .collect::<R<Vec<Vec3>>>()?;
    let found = query::evaluate(model, field(definition, "topologies"))?;
    let shapes: Vec<Shape> = found.iter().map(|r| dimension(model, *r).1).collect();
    let all = values::opt_bool(definition, "allDeviations")?.unwrap_or(false);
    let mut out: Vec<(f64, Vec3, Vec3)> = Vec::new();
    for p in points {
        let vertex = occt::vertex(p).map_err(occt_error)?;
        let mut best: Option<(f64, Vec3)> = None;
        for shape in &shapes {
            let d = vertex.distance(shape).map_err(occt_error)?;
            if best.is_none_or(|(b, _)| d.distance < b) {
                best = Some((d.distance, d.b));
            }
        }
        let (deviation, nearest) = best.ok_or_else(|| crate::regen_error("CANNOT_RESOLVE_ENTITIES"))?;
        out.push((deviation, p, nearest));
    }
    if !all {
        let farthest = out.iter().copied().fold(None::<(f64, Vec3, Vec3)>, |a, x| match a {
            Some(a) if a.0 >= x.0 => Some(a),
            _ => Some(x),
        });
        out = farthest.into_iter().collect();
    }
    Ok(Value::array(
        out.into_iter()
            .map(|(d, p, q)| {
                map(vec![
                    ("deviation", tags.length(d)),
                    ("pointPoint", tags.length_vector(p)),
                    ("topologyPoint", tags.length_vector(q)),
                ])
            })
            .collect(),
    ))
}

/// A B-spline curve's points and derivatives at parameters (in its knots' range), in metres: one array per order.
fn evaluate_spline(definition: &Value) -> R<Value> {
    let spline = field(definition, "spline");
    let degree = field(spline, "degree").as_number().ok_or_else(|| error("@evaluateSpline: degree"))? as usize;
    let poles = field(spline, "controlPoints")
        .as_array()
        .ok_or_else(|| error("@evaluateSpline: controlPoints"))?
        .iter()
        .map(|p| values::vec3(p, "evaluateSpline control point"))
        .collect::<R<Vec<Vec3>>>()?;
    let weights = if field(spline, "isRational").as_bool() == Some(true) {
        values::numbers(field(spline, "weights"), "evaluateSpline weights")?
    } else {
        vec![1.0; poles.len()]
    };
    let knots = values::numbers(field(spline, "knots"), "evaluateSpline knots")?;
    let parameters = values::numbers(field(definition, "parameters"), "evaluateSpline parameters")?;
    let orders = field(definition, "nDerivatives").as_number().unwrap_or(0.0) as usize;
    if poles.len() != weights.len() || knots.len() != poles.len() + degree + 1 {
        return Err(error("@evaluateSpline: the knots don't fit the control points"));
    }
    let mut out: Vec<Vec<Value>> = vec![Vec::new(); orders + 1];
    for &u in &parameters {
        for (order, point) in spline_derivatives(degree, &poles, &weights, &knots, u, orders).into_iter().enumerate() {
            out[order].push(Value::array(vec![Value::number(point.x), Value::number(point.y), Value::number(point.z)]));
        }
    }
    Ok(Value::array(out.into_iter().map(Value::array).collect()))
}

/// Points and derivatives of a (rational) B-spline at `u` (The NURBS Book, A2.3 and A4.2).
fn spline_derivatives(p: usize, poles: &[Vec3], weights: &[f64], knots: &[f64], u: f64, orders: usize) -> Vec<Vec3> {
    let n = poles.len() - 1;
    // The span: knots[k] <= u < knots[k + 1], the last one at the domain's end.
    let mut k = p;
    while k < n && u >= knots[k + 1] {
        k += 1;
    }
    // Basis functions' derivatives.
    let mut ndu = vec![vec![0.0; p + 1]; p + 1];
    ndu[0][0] = 1.0;
    let (mut left, mut right) = (vec![0.0; p + 1], vec![0.0; p + 1]);
    for j in 1..=p {
        left[j] = u - knots[k + 1 - j];
        right[j] = knots[k + j] - u;
        let mut saved = 0.0;
        for r in 0..j {
            ndu[j][r] = right[r + 1] + left[j - r];
            let temp = if ndu[j][r] != 0.0 { ndu[r][j - 1] / ndu[j][r] } else { 0.0 };
            ndu[r][j] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        ndu[j][j] = saved;
    }
    let mut ders = vec![vec![0.0; p + 1]; orders + 1];
    for j in 0..=p {
        ders[0][j] = ndu[j][p];
    }
    for r in 0..=p {
        let (mut s1, mut s2) = (0usize, 1usize);
        let mut a = vec![vec![0.0; p + 1]; 2];
        a[0][0] = 1.0;
        for d in 1..=orders.min(p) {
            let mut value = 0.0;
            let (rk, pk) = (r as isize - d as isize, (p - d) as isize);
            if r >= d {
                a[s2][0] = a[s1][0] / ndu[(pk + 1) as usize][rk as usize];
                value = a[s2][0] * ndu[rk as usize][pk as usize];
            }
            let j1 = if rk >= -1 { 1 } else { (-rk) as usize };
            let j2 = if (r as isize - 1) <= pk { d - 1 } else { p - r };
            for j in j1..=j2 {
                let idx = (rk + j as isize) as usize;
                a[s2][j] = (a[s1][j] - a[s1][j - 1]) / ndu[(pk + 1) as usize][idx];
                value += a[s2][j] * ndu[idx][pk as usize];
            }
            if r <= pk as usize {
                a[s2][d] = -a[s1][d - 1] / ndu[(pk + 1) as usize][r];
                value += a[s2][d] * ndu[r][pk as usize];
            }
            ders[d][r] = value;
            std::mem::swap(&mut s1, &mut s2);
        }
    }
    let mut factor = p as f64;
    for d in 1..=orders.min(p) {
        for j in 0..=p {
            ders[d][j] *= factor;
        }
        factor *= (p - d) as f64;
    }
    // The homogeneous curve's derivatives, then the rational curve's.
    let mut a = vec![Vec3::new(0.0, 0.0, 0.0); orders + 1];
    let mut w = vec![0.0; orders + 1];
    for d in 0..=orders.min(p) {
        for j in 0..=p {
            let i = k - p + j;
            a[d] = values::add(a[d], values::scale(poles[i], ders[d][j] * weights[i]));
            w[d] += ders[d][j] * weights[i];
        }
    }
    let mut out: Vec<Vec3> = Vec::with_capacity(orders + 1);
    for d in 0..=orders {
        let mut v = a[d];
        for i in 1..=d {
            let binomial = (1..=i).fold(1.0, |acc, t| acc * (d + 1 - t) as f64 / t as f64);
            v = values::sub(v, values::scale(out[d - i], binomial * w[i]));
        }
        out.push(values::scale(v, 1.0 / w[0]));
    }
    out
}

/// A face as a B-spline surface with its trimming boundary in its parameters. A whole cylinder is Onshape's exact
/// form (batches 100 c009, 101 c008): two rational cubic half circles round its z from its x (the world's y × z),
/// flagged periodic though their knots are clamped, along z from the face's start, bounded by six straight
/// parameter lines. Other faces are OCCT's conversion of their surface, without boundaries.
fn approximate_surface(tags: &Tags, model: &Model, definition: &Value) -> R<Value> {
    let found = query::evaluate(model, field(definition, "face"))?;
    let r = found
        .iter()
        .find(|r| r.kind == EntityType::Face)
        .ok_or_else(|| crate::regen_error("CANNOT_RESOLVE_ENTITIES"))?;
    let face = model.shape(*r);
    let numbers = |xs: &[f64]| Value::array(xs.iter().map(|&x| Value::number(x)).collect());
    let line = |a: (f64, f64), b: (f64, f64)| -> R<Value> {
        tags.tagged(
            map(vec![
                ("degree", Value::number(1.0)),
                ("dimension", Value::number(2.0)),
                ("isRational", Value::bool(false)),
                ("isPeriodic", Value::bool(false)),
                ("controlPoints", Value::array(vec![numbers(&[a.0, a.1]), numbers(&[b.0, b.1])])),
                ("knots", tags.tagged(numbers(&[0.0, 0.0, 1.0, 1.0]), "KnotArray")?),
            ]),
            "BSplineCurve",
        )
    };
    let s = face.surface_definition().map_err(occt_error)?;
    let ((u0, _), (u1, _)) = face.face_uv_bounds().map_err(occt_error)?;
    let whole = u1 - u0 > std::f64::consts::TAU - 1e-9;
    if let (1, true, Some(frame)) = (s.kind, whole, model.info(*r).surface_frame) {
        let z = frame.normal;
        let x = crate::ops::frames::world_y_cross(z).ok_or_else(|| error("@evApproximateBSplineSurface: no x"))?;
        let y = values::cross(z, x);
        let radius = s.radius1;
        let axis = s.frame.origin;
        let heights: Vec<f64> = face
            .sub_shapes(occt::ShapeKind::Vertex)
            .iter()
            .filter_map(|v| v.vertex_point().ok())
            .map(|p| values::dot(values::sub(p, axis), z))
            .collect();
        let (t0, t1) = heights.iter().fold((f64::MAX, f64::MIN), |(a, b), &t| (a.min(t), b.max(t)));
        let circle = [(1.0, 0.0), (1.0, 2.0), (-1.0, 2.0), (-1.0, 0.0), (-1.0, -2.0), (1.0, -2.0), (1.0, 0.0)];
        let mut poles = Vec::new();
        let mut weights = Vec::new();
        for (i, (a, b)) in circle.iter().enumerate() {
            let weight = if i % 3 == 0 { 1.0 } else { 1.0 / 3.0 };
            for t in [t0, t1] {
                let radial = values::add(values::scale(x, a * radius), values::scale(y, b * radius));
                poles.push(values::add(values::add(axis, values::scale(z, t)), radial));
                weights.push(weight);
            }
        }
        let data = occt::BSplineSurfaceData {
            u_degree: 3,
            v_degree: 1,
            u_periodic: false,
            v_periodic: false,
            rational: true,
            nu: 7,
            nv: 2,
            poles,
            weights,
            u_knots: vec![0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0],
            v_knots: vec![0.0, 0.0, 1.0, 1.0],
        };
        let surface = crate::bspline::bspline_surface_value(tags, &data)?;
        let mut entries = surface.as_map().map(|m| (**m).clone()).unwrap_or_default();
        entries.insert(Value::string("isUPeriodic"), Value::bool(true));
        let surface = Value::map(entries).with_tag(surface.tag);
        let corners = [(0.0, 0.0), (0.5, 0.0), (1.0, 0.0), (1.0, 1.0), (0.5, 1.0), (0.0, 1.0)];
        let boundaries = (0..6).map(|k| line(corners[k], corners[(k + 1) % 6])).collect::<R<Vec<Value>>>()?;
        return Ok(map(vec![
            ("bSplineSurface", surface),
            ("boundaryBSplineCurves", Value::array(boundaries)),
            ("innerLoopBSplineCurves", Value::array(Vec::new())),
        ]));
    }
    let data = face.face_bspline().map_err(occt_error)?;
    Ok(map(vec![
        ("bSplineSurface", crate::bspline::bspline_surface_value(tags, &data)?),
        ("boundaryBSplineCurves", Value::array(Vec::new())),
        ("innerLoopBSplineCurves", Value::array(Vec::new())),
    ]))
}
