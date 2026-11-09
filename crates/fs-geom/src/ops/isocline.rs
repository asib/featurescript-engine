//! Isoclines: the curves on faces where the tangent plane makes a given angle with a direction, like a draft
//! angle (`@opCreateIsocline`, `@opSplitByIsocline`). Spheres, and tori about an axis along the direction, have
//! them on planes square to the direction (batch 007, c026–c027); cylinders and cones across it along their length
//! (batches 030 and 034); a whole torus tilted to the direction has two closed curves round it (batch 057); on
//! B-spline faces they are traced (batch 065); other faces aren't supported yet.

use std::rc::Rc;

use super::{by_body, replace_bodies};
use crate::model::{BodyType, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::query::{self, tid_string};
use crate::values::{self, add, cross, dot, field, map, norm, normalize, scale, sub};
use crate::{occt_error, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Frame, Shape, Vec3};

/// A face's outward normal at a point on it.
fn normal_at(face: &Shape, p: Vec3) -> Option<Vec3> {
    let (fu, fv) = face.face_fraction(p).ok()?;
    face.face_tangent_plane(fu, fv).ok().map(|f| f.normal)
}

/// The planes on which a face's isoclines at `angle` from `direction` lie.
fn isocline_planes(face: &Shape, direction: Vec3, angle: f64) -> R<Vec<Frame>> {
    let s = face.surface_definition().map_err(occt_error)?;
    let p = face.face_inner_point().map_err(occt_error)?;
    let n = normal_at(face, p).ok_or_else(|| regen_error("ISOCLINE_NO_RESULT"))?;
    let target = angle.sin();
    let plane_at = |origin: Vec3| Frame { origin, normal: direction, x: values::canonical_x(direction) };
    match s.kind {
        // Sphere: the normal is (p − c)/R (or its opposite), so the isocline is where (p − c)·d = ±R sin a.
        3 => {
            let (c, radius) = (s.frame.origin, s.radius1);
            let sign = if dot(n, sub(p, c)) >= 0.0 { 1.0 } else { -1.0 };
            let h = sign * radius * target;
            Ok(if h.abs() < radius { vec![plane_at(add(c, scale(direction, h)))] } else { Vec::new() })
        }
        // Torus about an axis along the direction: the normal at tube angle θ rises at sin θ, so both circles at
        // height r sin θ = ±r sin a are isoclines.
        4 => {
            let (c, a, major, minor) = (s.frame.origin, s.frame.normal, s.radius1, s.radius2);
            if norm(cross(a, direction)) > 1e-9 {
                return Err(error("@opCreateIsocline: tori tilted to the direction aren't supported yet"));
            }
            let rel = sub(p, c);
            let radial = normalize(sub(rel, scale(a, dot(rel, a)))).ok_or_else(|| regen_error("ISOCLINE_NO_RESULT"))?;
            let tube = add(c, scale(radial, major));
            let sign = if dot(n, sub(p, tube)) >= 0.0 { 1.0 } else { -1.0 };
            let h = sign * minor * target * dot(a, direction).signum();
            Ok(if h.abs() < minor { vec![plane_at(add(c, scale(a, h)))] } else { Vec::new() })
        }
        // Planes, and cylinders and cones about the direction, have the same angle everywhere.
        0 => Ok(Vec::new()),
        1 | 2 if norm(cross(s.frame.normal, direction)) < 1e-9 => Ok(Vec::new()),
        _ => Err(error("@opCreateIsocline: isoclines of this kind of face aren't supported yet")),
    }
}

/// A cylinder across the direction has its isoclines along it: where its normal n(φ) = s(cos φ u + sin φ v) has
/// n·d = sin a, at φ = φ₀ ± acos(s sin a / |d⊥|) round the axis (batch 030). Half planes from the axis through
/// those lines cut them out.
fn cylinder_cutters(face: &Shape, direction: Vec3, angle: f64, size: f64) -> R<Option<Vec<Shape>>> {
    let s = face.surface_definition().map_err(occt_error)?;
    if s.kind != 1 || norm(cross(s.frame.normal, direction)) < 1e-9 {
        return Ok(None);
    }
    let (o, a, u) = (s.frame.origin, s.frame.normal, s.frame.x);
    let v = cross(a, u);
    let p = face.face_inner_point().map_err(occt_error)?;
    let n = normal_at(face, p).ok_or_else(|| regen_error("ISOCLINE_NO_RESULT"))?;
    let rel = sub(p, o);
    let sign = if dot(n, sub(rel, scale(a, dot(rel, a)))) >= 0.0 { 1.0 } else { -1.0 };
    let (du, dv) = (dot(direction, u), dot(direction, v));
    let across = du.hypot(dv);
    let c = sign * angle.sin() / across;
    if c.abs() > 1.0 {
        return Ok(Some(Vec::new()));
    }
    let base = dv.atan2(du);
    let spread = c.acos();
    let phis = if spread.abs() < 1e-12 { vec![base] } else { vec![base - spread, base + spread] };
    let mut out = Vec::new();
    for phi in phis {
        let radial = add(scale(u, phi.cos()), scale(v, phi.sin()));
        let frame = Frame { origin: o, normal: cross(a, radial), x: radial };
        out.push(occt::plane_face(frame, 0.0, size, -size, size).map_err(occt_error)?);
    }
    Ok(Some(out))
}

/// A cone across the direction has its isoclines along generators: with the cone opening along a from its apex at
/// half angle α, its outward normal at φ round the axis is cos α r(φ) − sin α a, so (signed by the face's side)
/// r(φ)·d = (s sin θ + sin α (a·d)) / cos α picks φ = φ₀ ± acos of that over |d⊥| (batch 034).
fn cone_cutters(face: &Shape, direction: Vec3, angle: f64, size: f64) -> R<Option<Vec<Shape>>> {
    let s = face.surface_definition().map_err(occt_error)?;
    if s.kind != 2 || norm(cross(s.frame.normal, direction)) < 1e-9 {
        return Ok(None);
    }
    let (apex, half) = (s.frame.origin, s.radius1);
    let p = face.face_inner_point().map_err(occt_error)?;
    let n = normal_at(face, p).ok_or_else(|| regen_error("ISOCLINE_NO_RESULT"))?;
    // The way the cone opens from its apex, and the frame round its axis.
    let a = if dot(sub(p, apex), s.frame.normal) >= 0.0 { s.frame.normal } else { scale(s.frame.normal, -1.0) };
    let u = normalize(sub(s.frame.x, scale(a, dot(s.frame.x, a)))).ok_or_else(|| regen_error("ISOCLINE_NO_RESULT"))?;
    let v = cross(a, u);
    let rel = sub(p, apex);
    let radial = normalize(sub(rel, scale(a, dot(rel, a)))).ok_or_else(|| regen_error("ISOCLINE_NO_RESULT"))?;
    let outward = add(scale(radial, half.cos()), scale(a, -half.sin()));
    let sign = if dot(n, outward) >= 0.0 { 1.0 } else { -1.0 };
    let (du, dv) = (dot(direction, u), dot(direction, v));
    let across = du.hypot(dv);
    let c = (sign * angle.sin() + half.sin() * dot(a, direction)) / half.cos() / across;
    if c.abs() > 1.0 {
        return Ok(Some(Vec::new()));
    }
    let base = dv.atan2(du);
    let spread = c.acos();
    let phis = if spread.abs() < 1e-12 { vec![base] } else { vec![base - spread, base + spread] };
    let mut out = Vec::new();
    for phi in phis {
        let r = add(scale(u, phi.cos()), scale(v, phi.sin()));
        let frame = Frame { origin: apex, normal: cross(a, r), x: r };
        out.push(occt::plane_face(frame, 0.0, size, -size, size).map_err(occt_error)?);
    }
    Ok(Some(out))
}

/// A whole torus tilted to the direction (batch 057 c013): its outward normal at angle θ round the axis and φ round
/// the tube is cos φ r(θ) + sin φ a, so n·d = s sin(angle) (signed by the face's side) is A(θ) cos φ + B sin φ = c
/// with A = d·r(θ), B = d·a: for each θ two tube angles, φ₀(θ) ± acos(c / |(A, B)|), two closed curves round the
/// torus. Each is the cubic Hermite spline through 128 of its points with its exact derivatives (dφ/dθ from the
/// equation). `None` if the face isn't a whole torus tilted to the direction, or the curves don't go all the way
/// round.
fn tilted_torus_isoclines(face: &Shape, direction: Vec3, angle: f64) -> R<Option<Vec<Shape>>> {
    let Some(curves) = tilted_torus_samples(face, direction, angle)? else { return Ok(None) };
    let parameters = sample_parameters();
    let mut out = Vec::new();
    for samples in curves {
        let points: Vec<Vec3> = samples.iter().map(|s| s.0).collect();
        let tangents: Vec<Vec3> = samples.iter().map(|s| s.1).collect();
        out.push(occt::interpolated_edge(&points, &tangents, &parameters, true).map_err(occt_error)?);
    }
    Ok(Some(out))
}

const TORUS_SAMPLES: usize = 128;

fn sample_parameters() -> Vec<f64> {
    (0..=TORUS_SAMPLES).map(|i| 2.0 * std::f64::consts::PI * i as f64 / TORUS_SAMPLES as f64).collect()
}

/// Ribbons standing across a tilted torus along its isoclines, to split it with (batch 058 c005): each the surface
/// between the curve carried a little way out along the torus's normal and a little way in.
fn tilted_torus_cutters(face: &Shape, direction: Vec3, angle: f64) -> R<Option<Vec<Shape>>> {
    let Some(curves) = tilted_torus_samples(face, direction, angle)? else { return Ok(None) };
    let s = face.surface_definition().map_err(occt_error)?;
    let width = 0.3 * s.radius2;
    let parameters = sample_parameters();
    let mut out = Vec::new();
    for samples in curves {
        let mut sides = Vec::new();
        for w in [width, -width] {
            let points: Vec<Vec3> = samples.iter().map(|(p, _, n, _)| add(*p, scale(*n, w))).collect();
            let tangents: Vec<Vec3> = samples.iter().map(|(_, t, _, dn)| add(*t, scale(*dn, w))).collect();
            let edge = occt::interpolated_edge(&points, &tangents, &parameters, true).map_err(occt_error)?;
            sides.extend(occt::connect_edges(&[edge]).map_err(occt_error)?);
        }
        let ribbon = occt::loft(&sides, false).map_err(|_| regen_error("ISOCLINE_NO_RESULT"))?.shape();
        out.extend(ribbon.sub_shapes(occt::ShapeKind::Face));
    }
    Ok(Some(out))
}

/// Points along a whole torus's isoclines when it is tilted to the direction (see [`tilted_torus_isoclines`]):
/// for each curve, at each sample, the point, its derivative along the angle round the axis, the outward normal
/// and the normal's derivative.
#[allow(clippy::type_complexity)]
fn tilted_torus_samples(face: &Shape, direction: Vec3, angle: f64) -> R<Option<Vec<Vec<(Vec3, Vec3, Vec3, Vec3)>>>> {
    Ok(tilted_torus_samples_over(face, direction, angle, false)?.map(|(curves, _)| curves))
}

/// [`tilted_torus_samples`], also on part of a torus round its axis (the whole way round the tube) if `partial`:
/// then each curve runs from one end of the face's angle round the axis to the other, sampled at both ends, and
/// the angles of the samples come too.
#[allow(clippy::type_complexity)]
fn tilted_torus_samples_over(
    face: &Shape,
    direction: Vec3,
    angle: f64,
    partial: bool,
) -> R<Option<(Vec<Vec<(Vec3, Vec3, Vec3, Vec3)>>, Vec<f64>)>> {
    let s = face.surface_definition().map_err(occt_error)?;
    if s.kind != 4 || norm(cross(s.frame.normal, direction)) < 1e-9 {
        return Ok(None);
    }
    let ((u0, v0), (u1, v1)) = face.face_uv_bounds().map_err(occt_error)?;
    let full = |a: f64, b: f64| ((b - a) - 2.0 * std::f64::consts::PI).abs() < 1e-9;
    let whole = full(u0, u1);
    if !full(v0, v1) || !(whole || partial) {
        return Ok(None);
    }
    let (c0, a, u) = (s.frame.origin, s.frame.normal, s.frame.x);
    let v = cross(a, u);
    let (major, minor) = (s.radius1, s.radius2);
    let p = face.face_inner_point().map_err(occt_error)?;
    let n = normal_at(face, p).ok_or_else(|| regen_error("ISOCLINE_NO_RESULT"))?;
    let rel = sub(p, c0);
    let radial = normalize(sub(rel, scale(a, dot(rel, a)))).ok_or_else(|| regen_error("ISOCLINE_NO_RESULT"))?;
    let sign = if dot(n, sub(p, add(c0, scale(radial, major)))) >= 0.0 { 1.0 } else { -1.0 };
    let c = sign * angle.sin();
    let (du, dv, da) = (dot(direction, u), dot(direction, v), dot(direction, a));
    let samples = TORUS_SAMPLES;
    let mut curves: Vec<Vec<(Vec3, Vec3, Vec3, Vec3)>> = vec![Vec::new(), Vec::new()];
    // The angles round the axis, in the face's parameter (its frame's x, towards its y).
    let thetas: Vec<f64> = if whole {
        (0..samples).map(|i| 2.0 * std::f64::consts::PI * i as f64 / samples as f64).collect()
    } else {
        (0..=samples).map(|i| u0 + (u1 - u0) * i as f64 / samples as f64).collect()
    };
    for &th in &thetas {
        let big_a = du * th.cos() + dv * th.sin();
        let reach = big_a.hypot(da);
        if c.abs() >= reach {
            return Ok(None);
        }
        let base = da.atan2(big_a);
        for (k, branch) in [1.0, -1.0].into_iter().enumerate() {
            let phi = base + branch * (c / reach).acos();
            // F(θ, φ) = A cos φ + B sin φ − c; dφ/dθ = −F_θ / F_φ.
            let f_th = phi.cos() * (-du * th.sin() + dv * th.cos());
            let f_phi = -phi.sin() * big_a + phi.cos() * da;
            let dphi = -f_th / f_phi;
            let r_th = add(scale(u, th.cos()), scale(v, th.sin()));
            let t_th = add(scale(u, -th.sin()), scale(v, th.cos()));
            let rho = major + minor * phi.cos();
            let point = add(c0, add(scale(r_th, rho), scale(a, minor * phi.sin())));
            // dp/dθ = ρ t(θ) + (−r sin φ r(θ) + r cos φ a) dφ/dθ.
            let tangent =
                add(scale(t_th, rho), scale(add(scale(r_th, -minor * phi.sin()), scale(a, minor * phi.cos())), dphi));
            // n = cos φ r(θ) + sin φ a; dn/dθ = cos φ t(θ) + (−sin φ r(θ) + cos φ a) dφ/dθ.
            let normal = add(scale(r_th, phi.cos()), scale(a, phi.sin()));
            let dnormal = add(scale(t_th, phi.cos()), scale(add(scale(r_th, -phi.sin()), scale(a, phi.cos())), dphi));
            curves[k].push((point, tangent, normal, dnormal));
        }
    }
    Ok(Some((curves, thetas)))
}

/// The isoclines of part of a torus round its axis, tilted to the direction (batch 070 c005): open curves from one
/// end of the face to the other, each running along n × d (the face's outward normal crossed with the direction),
/// as Onshape's do.
fn partial_tilted_torus_isoclines(face: &Shape, direction: Vec3, angle: f64) -> R<Option<Vec<Shape>>> {
    let Some((curves, thetas)) = tilted_torus_samples_over(face, direction, angle, true)? else { return Ok(None) };
    if thetas.len() != TORUS_SAMPLES + 1 {
        return Ok(None);
    }
    let mut out = Vec::new();
    for samples in curves {
        let mut points: Vec<Vec3> = samples.iter().map(|s| s.0).collect();
        let mut tangents: Vec<Vec3> = samples.iter().map(|s| s.1).collect();
        let mut parameters = thetas.clone();
        let (_, t, n, _) = samples[samples.len() / 2];
        if dot(t, cross(n, direction)) < 0.0 {
            points.reverse();
            tangents = tangents.into_iter().rev().map(|t| scale(t, -1.0)).collect();
            parameters = parameters.iter().rev().map(|p| -p).collect();
        }
        out.push(occt::interpolated_edge(&points, &tangents, &parameters, false).map_err(occt_error)?);
    }
    Ok(Some(out))
}

/// Large planar faces standing in for the isocline planes of some faces.
fn isocline_cutters(faces: &[Shape], direction: Vec3, angle: f64) -> R<Vec<Shape>> {
    let mut out = Vec::new();
    for face in faces {
        let bounds = face.bounding_box(false).map_err(occt_error)?;
        let size = norm(sub(bounds.max, bounds.min)) * 2.0 + 1e-3;
        if let Some(cutters) = cylinder_cutters(face, direction, angle, size)? {
            out.extend(cutters);
            continue;
        }
        if let Some(cutters) = tilted_torus_cutters(face, direction, angle)? {
            out.extend(cutters);
            continue;
        }
        if let Some(cutters) = cone_cutters(face, direction, angle, size + norm(sub(bounds.max, bounds.min)) * 4.0)? {
            out.extend(cutters);
            continue;
        }
        for plane in isocline_planes(face, direction, angle)? {
            out.push(occt::plane_face(plane, -size, size, -size, size).map_err(occt_error)?);
        }
    }
    Ok(out)
}

fn direction_and_angle(definition: &Value) -> R<(Vec3, f64)> {
    let direction = normalize(values::vec3(field(definition, "direction"), "isocline direction")?)
        .ok_or_else(|| regen_error("ISOCLINE_SELECT_DIRECTION"))?;
    Ok((direction, values::angle(field(definition, "angle"), "isocline angle")?))
}

/// The isocline across a face with no analytic one (a B-spline, batch 065 c003), traced: along whichever of its
/// parameters every line of the other crosses the angle once, the crossing found on each and a spline put through
/// them. It runs along direction × normal. None if no such crossings.
fn traced_isocline(face: &Shape, direction: Vec3, angle: f64) -> R<Option<Shape>> {
    Ok(traced_points(face, direction, angle)?.map(|(edge, _)| edge))
}

/// [`traced_isocline`]'s edge, with the fractions (u, v) of the face's parameter ranges it passes through.
fn traced_points(face: &Shape, direction: Vec3, angle: f64) -> R<Option<(Shape, Vec<(f64, f64)>)>> {
    const LINES: usize = 64;
    const STEPS: usize = 64;
    let target = angle.sin();
    let at = |fu: f64, fv: f64, across_u: bool| {
        let (u, v) = if across_u { (fv, fu) } else { (fu, fv) };
        face.face_tangent_plane(u, v).ok()
    };
    for across_u in [false, true] {
        let mut points = Vec::new();
        let mut fractions = Vec::new();
        for i in 0..=LINES {
            let s = i as f64 / LINES as f64;
            let f = |t: f64| at(s, t, across_u).map(|p| dot(p.normal, direction) - target);
            let mut crossing = None;
            let mut count = 0;
            let mut prev = f(0.0);
            for k in 1..=STEPS {
                let t = k as f64 / STEPS as f64;
                let here = f(t);
                if let (Some(a), Some(b)) = (prev, here)
                    && (a < 0.0) != (b < 0.0)
                {
                    count += 1;
                    let (mut lo, mut hi) = ((k - 1) as f64 / STEPS as f64, t);
                    for _ in 0..60 {
                        let mid = (lo + hi) / 2.0;
                        match f(mid) {
                            Some(m) if (m < 0.0) == (a < 0.0) => lo = mid,
                            Some(_) => hi = mid,
                            None => break,
                        }
                    }
                    crossing = Some((lo + hi) / 2.0);
                }
                prev = here;
            }
            match (count, crossing) {
                (1, Some(t)) => {
                    points.push(at(s, t, across_u).ok_or_else(|| regen_error("ISOCLINE_NO_RESULT"))?);
                    fractions.push(if across_u { (t, s) } else { (s, t) });
                }
                _ => {
                    points.clear();
                    break;
                }
            }
        }
        if points.len() < 2 {
            continue;
        }
        // Along direction × normal.
        let middle = &points[points.len() / 2];
        let ahead = sub(points[points.len() / 2 + 1].origin, middle.origin);
        if dot(ahead, cross(direction, middle.normal)) < 0.0 {
            points.reverse();
            fractions.reverse();
        }
        let positions: Vec<Vec3> = points.iter().map(|p| p.origin).collect();
        let mut parameters = vec![0.0];
        for w in positions.windows(2) {
            parameters.push(parameters.last().copied().unwrap_or(0.0) + norm(sub(w[1], w[0])));
        }
        let n = positions.len();
        let derivatives: Vec<Vec3> = (0..n)
            .map(|k| {
                let (a, b) = if k == 0 { (0, 1) } else if k == n - 1 { (n - 2, n - 1) } else { (k - 1, k + 1) };
                scale(sub(positions[b], positions[a]), 1.0 / (parameters[b] - parameters[a]))
            })
            .collect();
        let edge = occt::interpolated_edge(&positions, &derivatives, &parameters, false).map_err(occt_error)?;
        return Ok(Some((edge, fractions)));
    }
    Ok(None)
}

/// `@opCreateIsocline`: a wire body for each isocline.
pub fn create(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let (direction, angle) = direction_and_angle(definition)?;
    let faces: Vec<Shape> = query::evaluate(model, field(definition, "faces"))?
        .into_iter()
        .filter(|r| r.kind == EntityType::Face)
        .map(|r| model.shape(r).clone())
        .collect();
    let info = Info::new(id);
    // Traced isoclines join up where they meet (the sides of a loft, one wire round it, batch 065 c003).
    let mut traced = Vec::new();
    for face in &faces {
        if matches!(face.surface_type(), Ok(occt::SurfaceType::BSpline | occt::SurfaceType::Bezier)) {
            if let Some(edge) = traced_isocline(face, direction, angle)? {
                traced.push(edge);
            }
            continue;
        }
        let curves = match tilted_torus_isoclines(face, direction, angle)? {
            Some(curves) => Some(curves),
            None => partial_tilted_torus_isoclines(face, direction, angle)?,
        };
        if let Some(curves) = curves {
            for edge in curves {
                for wire in occt::connect_edges(&[edge]).map_err(occt_error)? {
                    model.add_body(
                        NewBody::new(wire, BodyType::Wire, info.clone()),
                        &Origins::default(),
                        &info,
                        &|_| None,
                    );
                }
            }
            continue;
        }
        let cutters = isocline_cutters(std::slice::from_ref(face), direction, angle)?;
        if cutters.is_empty() {
            continue;
        }
        let edges = occt::section_edges(std::slice::from_ref(face), &cutters).map_err(occt_error)?;
        for wire in occt::connect_edges(&edges).map_err(occt_error)? {
            model.add_body(NewBody::new(wire, BodyType::Wire, info.clone()), &Origins::default(), &info, &|_| None);
        }
    }
    if !traced.is_empty() {
        for wire in occt::connect_edges(&traced).map_err(occt_error)? {
            model.add_body(NewBody::new(wire, BodyType::Wire, info.clone()), &Origins::default(), &info, &|_| None);
        }
    }
    Ok(Value::UNDEFINED)
}

/// `@opSplitByIsocline`: the faces split along their isoclines. The pieces whose angle is at least the isocline
/// angle are in front of the split (`qSplitBy`), the steep ones behind. Returns transient ids of the steep and
/// other faces and the new edges.
pub fn split(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let (direction, angle) = direction_and_angle(definition)?;
    let groups = by_body(model, field(definition, "faces"), EntityType::Face)?;
    if groups.is_empty() {
        return Err(regen_error("ISOCLINE_SELECT_FACES"));
    }
    let mut results = Vec::new();
    let mut split_tids = Vec::new();
    let mut input_tids = Vec::new();
    for (b, faces) in groups {
        input_tids
            .extend(model.bodies[b].faces.iter().filter(|f| faces.iter().any(|g| g.is_same(&f.shape))).map(|f| f.tid));
        // A whole torus tilted to the direction: the two bands between its curves (batch 058 c005).
        if let [face] = &faces[..] {
            if model.bodies[b].faces.len() == 1 {
                if let Some(curves) = tilted_torus_samples(face, direction, angle)? {
                    let points: Vec<Vec<Vec3>> = curves.iter().map(|c| c.iter().map(|s| s.0).collect()).collect();
                    let op = occt::split_by_bands(&model.bodies[b].shape, face, &points[0], &points[1])
                        .map_err(|_| regen_error("ISOCLINE_NO_RESULT"))?;
                    split_tids.push(model.bodies[b].tid);
                    results.push((b, op));
                    continue;
                }
            }
        }
        // B-spline faces are cut along their traced isoclines, drawn on their surfaces, by the general splitter
        // (batch 066 c001: so neighbouring faces' cuts meet).
        if faces.iter().all(|f| matches!(f.surface_type(), Ok(occt::SurfaceType::BSpline | occt::SurfaceType::Bezier))) {
            let mut edges = Vec::new();
            for face in &faces {
                if let Some((_, fractions)) = traced_points(face, direction, angle)? {
                    edges.push(occt::edge_on_face(face, &fractions).map_err(occt_error)?);
                }
            }
            if edges.is_empty() {
                continue;
            }
            let op = occt::split(std::slice::from_ref(&model.bodies[b].shape), &edges)
                .map_err(|_| regen_error("ISOCLINE_NO_RESULT"))?;
            split_tids.push(model.bodies[b].tid);
            results.push((b, op));
            continue;
        }
        let cutters = isocline_cutters(&faces, direction, angle)?;
        if cutters.is_empty() {
            continue;
        }
        let op = occt::split_faces(&model.bodies[b].shape, &faces, &cutters)
            .map_err(|_| regen_error("ISOCLINE_NO_RESULT"))?;
        split_tids.push(model.bodies[b].tid);
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    let (mut steep, mut not_steep, mut boundary) = (Vec::new(), Vec::new(), Vec::new());
    let mut sides = Vec::new();
    for r in model.everything(Some(EntityType::Face)) {
        let entity = model.entity(r).expect("a face");
        let from_input =
            input_tids.contains(&entity.tid) || entity.ancestry.identity.iter().any(|t| input_tids.contains(t));
        if !from_input {
            continue;
        }
        let Some(n) = entity.shape.face_inner_point().ok().and_then(|p| normal_at(&entity.shape, p)) else { continue };
        let is_steep = dot(n, direction).asin() < angle;
        let transient = Value::string(tid_string(entity.tid));
        if is_steep {
            steep.push(transient)
        } else {
            not_steep.push(transient)
        }
        if !input_tids.contains(&entity.tid) {
            sides.push((id.clone(), entity.tid, is_steep));
        }
    }
    Rc::make_mut(&mut model.splits).extend(sides);
    for r in model.everything(Some(EntityType::Edge)) {
        if split_tids.contains(&model.bodies[r.body].tid) && model.info(r).is_created_by(&id) {
            boundary.push(Value::string(tid_string(model.tid(r))));
        }
    }
    Ok(map(vec![
        ("steepFaces", Value::array(steep)),
        ("nonSteepFaces", Value::array(not_steep)),
        ("boundaryEdges", Value::array(boundary)),
    ]))
}
