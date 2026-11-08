//! `@opFillet` and `@opChamfer`.

use super::{by_body, replace_bodies};
use crate::model::{EntityRef, EntityType, FeatureId, Model, Tid};
use crate::values::{self, field};
use crate::{query, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Shape, ShapeKind};

/// The edges to blend: edges selected, and every edge of faces selected, grouped by body.
fn blend_edges(model: &Model, definition: &Value) -> R<Vec<(usize, Vec<Shape>)>> {
    let entities = field(definition, "entities");
    let mut groups = by_body(model, entities, EntityType::Edge)?;
    for (b, faces) in by_body(model, entities, EntityType::Face)? {
        let edges: Vec<Shape> = faces.iter().flat_map(|f| f.visible_sub_shapes(ShapeKind::Edge)).collect();
        match groups.iter_mut().find(|(g, _)| *g == b) {
            Some((_, list)) => list.extend(edges),
            None => groups.push((b, edges)),
        }
    }
    for (_, edges) in &mut groups {
        let mut unique: Vec<Shape> = Vec::new();
        for e in edges.drain(..) {
            if !unique.iter().any(|u| u.is_same(&e)) {
                unique.push(e);
            }
        }
        *edges = unique;
    }
    Ok(groups)
}

pub fn fillet(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    match field(definition, "crossSection").as_str() {
        None | Some("CIRCULAR") => {}
        Some(kind @ ("CONIC" | "CURVATURE")) if !values::opt_bool(definition, "isVariable")?.unwrap_or(false) => {
            let key = if kind == "CONIC" { "rho" } else { "magnitude" };
            let value = field(definition, key)
                .as_number()
                .ok_or_else(|| error(format!("@opFillet: {key} must be a number")))?;
            if !(value > 0.0 && value < 1.0) {
                return Err(regen_error("FILLET_FAILED"));
            }
            let section = if kind == "CONIC" { Section::Conic(value) } else { Section::Curvature(value) };
            return section_fillet(model, id, definition, section);
        }
        Some(other) => return Err(error(format!("@opFillet: {other} cross sections aren't supported yet"))),
    }
    // Asymmetric with equal radii is a plain circular fillet: Onshape's is a cylinder (batch 052, c017).
    let length = |key: &str| values::length(field(definition, key), key).ok();
    let asymmetric = values::opt_bool(definition, "isAsymmetric")?.unwrap_or(false)
        && !matches!((length("radius"), length("otherRadius")), (Some(a), Some(b)) if (a - b).abs() < 1e-12);
    if asymmetric && !values::opt_bool(definition, "isVariable")?.unwrap_or(false) {
        return section_fillet(model, id, definition, Section::Circular);
    }
    for (key, on) in [
        ("isAsymmetric", asymmetric),
        ("createDetachedSurface", values::opt_bool(definition, "createDetachedSurface")?.unwrap_or(false)),
    ] {
        if on {
            return Err(error(format!("@opFillet: {key} isn't supported yet")));
        }
    }
    if values::opt_bool(definition, "isVariable")?.unwrap_or(false) {
        return variable_fillet(model, id, definition);
    }
    if field(definition, "partialFilletBounds").as_array().is_some_and(|b| !b.is_empty()) {
        return partial_fillet(model, id, definition);
    }
    let radius = values::length(field(definition, "radius"), "opFillet radius")?;
    let groups = blend_edges(model, definition)?;
    if groups.is_empty() {
        return Err(regen_error("FILLET_SELECT_EDGES"));
    }
    let mut results = Vec::new();
    let mut framed: Vec<(Shape, occt::Frame)> = Vec::new();
    for (b, edges) in groups {
        let body = model.bodies[b].shape.clone();
        let Ok(op) = occt::fillet(&body, &edges, radius) else {
            return section_fillet(model, id, definition, Section::Circular).map_err(|_| regen_error("FILLET_FAILED"));
        };
        for edge in &edges {
            let line_origin =
                model.bodies[b].edges.iter().find(|e| e.shape.is_same(edge)).and_then(|e| e.info.line_origin);
            for face in op.generated(edge) {
                if let Some(frame) = fillet_frame(&body, edge, &face, line_origin) {
                    framed.push((face, frame));
                }
            }
        }
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    // The fillets' cylinders get Onshape's coordinate systems.
    for (face, frame) in framed {
        let Some((b, i)) = model
            .bodies
            .iter()
            .enumerate()
            .find_map(|(b, body)| body.faces.iter().position(|f| f.shape.is_same(&face)).map(|i| (b, i)))
        else {
            continue;
        };
        model.update_body(b, |body| {
            let info = crate::model::Info { surface_frame: Some(frame), ..(*body.faces[i].info).clone() };
            body.faces[i].info = std::rc::Rc::new(info);
        });
    }
    Ok(Value::UNDEFINED)
}

/// A conic, curvature or asymmetric fillet of straight edges between planar faces (batches 003, 052, 053). Its
/// cross section runs from a point set back along one face to one set back along the other, round the edge. The
/// setbacks are the feet on the faces of a centre `d1` from face 1 and `d2` from face 2: `(d2 + d1 cos α) / sin α`
/// along face 1, α the angle between the faces. A conic or curvature fillet's centre is the circular fillet's
/// (`d1 = d2 =` its radius); an asymmetric fillet's is `radius` from the [`leading_face`] and `otherRadius` from
/// the other at a convex edge, the other way round at a concave one (flipped, swapped), and where that leaves a
/// setback negative (an obtuse corner, uneven radii) Onshape fails. The section is a rational quadratic with the
/// edge its middle control point, weighted `rho / (1 - rho)` (a conic) or `sin(α / 2)` (asymmetric: at a right
/// angle a quarter ellipse), made a cubic B-spline as Onshape does ([`Cubic::rational_quadratic`]); or a
/// curvature fillet's quintic ([`curvature_section`]). The region between the edge and the curve, swept along the
/// edge, is taken away (or, at a concave edge, filled in). Each edge's region is its own, so edges meeting at a
/// vertex aren't blended there.
///
/// A plain circular fillet too big for the faces it rounds (c015: a 30 × 20 block's upright edges rounded 15 mm)
/// comes here too, when OCCT's fillet fails: Onshape lets it eat the faces, leaving what taking each edge's region
/// away, with the exact arc, leaves (the arcs of neighbouring corners cross; corners rounded about one axis merge).
fn section_fillet(model: &mut Model, id: FeatureId, definition: &Value, section: Section) -> R<Value> {
    use crate::values::{add, cross, dot, norm, normalize, scale, sub};
    let key = if matches!(section, Section::Circular) { "radius" } else { "nonCircularRadius" };
    let radius = values::length(field(definition, key), "opFillet radius")?;
    let asymmetric = values::opt_bool(definition, "isAsymmetric")?.unwrap_or(false);
    let other =
        if asymmetric { values::length(field(definition, "otherRadius"), "opFillet otherRadius")? } else { radius };
    let (radius, other) = if values::opt_bool(definition, "flipAsymmetric")?.unwrap_or(false) {
        (other, radius)
    } else {
        (radius, other)
    };
    if radius <= 0.0 || other <= 0.0 {
        return Err(regen_error("FILLET_FAILED"));
    }
    let groups = blend_edges(model, definition)?;
    if groups.is_empty() {
        return Err(regen_error("FILLET_SELECT_EDGES"));
    }
    let failed = || regen_error("FILLET_FAILED");
    let mut results = Vec::new();
    for (b, edges) in groups {
        let body = model.bodies[b].shape.clone();
        let vertices: Vec<Shape> = edges.iter().flat_map(|e| e.sub_shapes(ShapeKind::Vertex)).collect();
        // Two edges meeting at a vertex end in a mitre, their regions overlapping; three or more need a blend there.
        if vertices.iter().any(|v| vertices.iter().filter(|w| w.is_same(v)).count() > 2) {
            return Err(error(
                "@opFillet: conic, curvature and asymmetric fillets of three edges meeting at a vertex aren't supported yet",
            ));
        }
        let (mut cut, mut fill) = (Vec::new(), Vec::new());
        for edge in &edges {
            if edge.curve_type().ok() != Some(occt::CurveType::Line) {
                return Err(error(
                    "@opFillet: conic, curvature and asymmetric fillets of curved edges aren't supported yet",
                ));
            }
            let (a, e) = (
                edge.edge_tangent(0.0, false).map_err(|_| failed())?,
                edge.edge_tangent(1.0, false).map_err(|_| failed())?,
            );
            let (start, length, t) = (a.origin, norm(sub(e.origin, a.origin)), a.direction);
            let mid = add(start, scale(t, length / 2.0));
            // Into each face from the edge, square to it: the leading face first.
            let mut faces = body.ancestors(edge, ShapeKind::Face);
            if faces.len() != 2 {
                return Err(failed());
            }
            if leading_face(&body, edge).is_some_and(|l| l.is_same(&faces[1])) {
                faces.swap(0, 1);
            }
            let mut into = Vec::new();
            for face in &faces {
                let plane = face.face_plane().map_err(|_| {
                    error(
                        "@opFillet: conic, curvature and asymmetric fillets between curved faces aren't supported yet",
                    )
                })?;
                let mut u = normalize(cross(plane.normal, t)).ok_or_else(failed)?;
                let on = |d: occt::Vec3| {
                    occt::vertex(add(mid, scale(d, 1e-6)))
                        .and_then(|v| v.distance(face))
                        .is_ok_and(|x| x.distance < 1e-7)
                };
                if !on(u) {
                    u = scale(u, -1.0);
                }
                into.push(u);
            }
            let [u1, u2] = into[..] else { return Err(failed()) };
            let angle = dot(u1, u2).clamp(-1.0, 1.0).acos();
            // Just inside the corner, halfway along the edge (its ends lie on the part's other faces).
            let inside = body.contains_point(add(mid, scale(add(u1, u2), radius.min(other) * 0.05))).unwrap_or(true);
            // How far the section's centre lies from each face: `radius` from the leading face at a convex edge,
            // from the other at a concave one; then where its feet on the faces are (none, past the edge, at an
            // obtuse corner with radii too uneven: c014).
            let (d1, d2) = if inside { (radius, other) } else { (other, radius) };
            let (s1, s2) = ((d2 + d1 * angle.cos()) / angle.sin(), (d1 + d2 * angle.cos()) / angle.sin());
            if !(s1 > 1e-9 && s2 > 1e-9) {
                return Err(failed());
            }
            // The section is swept along the edge: to cut, from a length before it to a length past it, so its ends
            // lie outside the part; to fill a concave edge, along just its length. It's built where the sweep
            // starts.
            let (corner, sweep) = if inside { (sub(start, scale(t, length)), 3.0 * length) } else { (start, length) };
            let (p1, p2) = (add(corner, scale(u1, s1)), add(corner, scale(u2, s2)));
            let start = corner;
            // From the second face to the first; a circular fillet's the exact arc.
            let curve = match section {
                Section::Circular if !asymmetric => {
                    let bisector = normalize(add(u1, u2)).ok_or_else(failed)?;
                    let centre = add(start, scale(bisector, radius / (angle / 2.0).sin()));
                    occt::arc_through(p2, sub(centre, scale(bisector, radius)), p1).map_err(crate::occt_error)?
                }
                Section::Circular => {
                    crate::curve_extend::Cubic::rational_quadratic(p2, start, p1, (angle / 2.0).sin())?.edge()?
                }
                Section::Conic(rho) => {
                    crate::curve_extend::Cubic::rational_quadratic(p2, start, p1, rho / (1.0 - rho))?.edge()?
                }
                Section::Curvature(m) => curvature_section(p2, start, p1, m)?,
            };
            let region = occt::fill_edges(&[
                curve,
                occt::line_edge(p1, start).map_err(crate::occt_error)?,
                occt::line_edge(start, p2).map_err(crate::occt_error)?,
            ])
            .map_err(|_| failed())?;
            let tool = occt::prism(&region, scale(t, sweep)).map_err(crate::occt_error)?.shape();
            // A B-spline section's sweep made a B-spline surface, as Onshape's is: OCCT intersects two such
            // surfaces (two fillets mitred at a corner) far better than two extrusions (batch 053 c013: their faces
            // 0.17 mm² short, against 0.03 once converted).
            let exact_arc = matches!(section, Section::Circular) && !asymmetric;
            let converted =
                if exact_arc { tool.clone() } else { occt::nurbs_converted(&tool).map_err(crate::occt_error)? };
            if inside { cut.push((converted, tool)) } else { fill.push((converted, tool)) }
        }
        // With the converted tools, OCCT's boolean can come back unchanged, reporting no error (batch 056 c017, a
        // curvature fillet of magnitude 0.82): then with the extrusions themselves.
        let apply = |kind: occt::BooleanKind, shape: &Shape, tools: &[(Shape, Shape)]| -> R<occt::Operation> {
            let before = shape.volume().unwrap_or(0.0);
            let converted: Vec<Shape> = tools.iter().map(|(c, _)| c.clone()).collect();
            let o = occt::boolean(kind, std::slice::from_ref(shape), &converted, true).map_err(|_| failed())?;
            if (o.shape().volume().unwrap_or(0.0) - before).abs() > 1e-15 {
                return Ok(o);
            }
            let raw: Vec<Shape> = tools.iter().map(|(_, r)| r.clone()).collect();
            occt::boolean(kind, std::slice::from_ref(shape), &raw, true).map_err(|_| failed())
        };
        let mut shape = body.clone();
        let mut op = None;
        if !cut.is_empty() {
            let o = apply(occt::BooleanKind::Subtraction, &shape, &cut)?;
            shape = o.shape();
            op = Some(o);
        }
        if !fill.is_empty() {
            op = Some(apply(occt::BooleanKind::Union, &shape, &fill)?);
        }
        if let Some(op) = op {
            results.push((b, op));
        }
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

/// The coordinate system Onshape gives the cylinder a fillet of a straight edge makes (batch 047): z along the edge
/// as it runs in the loop of its neighbouring face whose normal leans most along x (then y, then z), x along y × z
/// (x × z for an edge along y), and the origin where the edge's line is placed (`Info::line_origin`, else its middle)
/// brought onto the axis.
fn fillet_frame(body: &Shape, edge: &Shape, face: &Shape, line_origin: Option<occt::Vec3>) -> Option<occt::Frame> {
    use crate::values::{add, cross, dot, normalize, scale, sub};
    let s = face.surface_definition().ok()?;
    if s.kind != 1 || edge.curve_type().ok() != Some(occt::CurveType::Line) {
        return None;
    }
    let mid = edge.edge_tangent(0.5, false).ok()?;
    let (m, t) = (mid.origin, mid.direction);
    let normal_of = |f: &Shape| -> Option<occt::Vec3> {
        let (u, v) = f.face_fraction(m).ok()?;
        f.face_tangent_plane(u, v).ok().map(|p| p.normal)
    };
    let chosen = leading_face(body, edge)?;
    let n = normal_of(&chosen)?;
    let chosen = &chosen;
    // Walking the face's loop (anticlockwise about its normal), the face is on the left.
    let left = cross(n, t);
    let inside =
        occt::vertex(add(m, scale(left, 1e-6))).and_then(|v| v.distance(chosen)).is_ok_and(|d| d.distance < 1e-7);
    let along = if inside { t } else { scale(t, -1.0) };
    let z = if dot(s.frame.normal, along) > 0.0 { s.frame.normal } else { scale(s.frame.normal, -1.0) };
    let y_axis = occt::Vec3::new(0.0, 1.0, 0.0);
    let x = if dot(z, y_axis).abs() > 1.0 - 1e-9 {
        normalize(cross(occt::Vec3::new(1.0, 0.0, 0.0), z))?
    } else {
        normalize(cross(y_axis, z))?
    };
    let p = line_origin.unwrap_or(m);
    let origin = add(s.frame.origin, scale(z, dot(sub(p, s.frame.origin), z)));
    Some(occt::Frame { origin, normal: z, x })
}

/// A fillet's cross section.
#[derive(Clone, Copy, Debug)]
enum Section {
    Circular,
    /// A conic of this rho.
    Conic(f64),
    /// Curvature continuous, of this magnitude.
    Curvature(f64),
}

/// A curvature continuous fillet's cross section from `a` to `b` round the corner `c`, as Onshape makes it (batch
/// 053, to 1e-11 in every case): the quintic B-spline on knots 0⁶ 0.5² 1⁶ whose eight poles are, as `c + α (a − c)
/// + β (b − c)`, (1, 0), (1 − 0.3m, 0), (1 − 0.6m, 0), (α₃, β₃) and their mirror images, with
/// β₃ = 1/3 − 7m/15 and α₃ = 2(1 − m)/3: it starts and ends straight (no curvature, nor second derivative), passes
/// through the chord's middle moved `m` of the way to the corner, with no third derivative there. Past m = 5/7,
/// where that β₃ would be negative, it is a fortieth of that the other way, (7m − 5)/600, α₃ taking up the
/// difference through the shoulder (batches 053, 055, 056: m 0.72 to 0.86, exactly). Past that, where α₃ would fall
/// below β₃ (m about 0.87), Onshape moves the third pole too, by no rule found yet.
fn curvature_section(a: occt::Vec3, c: occt::Vec3, b: occt::Vec3, m: f64) -> R<Shape> {
    use crate::values::{add, scale, sub};
    let (mut alpha, mut beta) = (2.0 * (1.0 - m) / 3.0, 1.0 / 3.0 - 7.0 * m / 15.0);
    if beta < 0.0 {
        beta = -beta / 40.0;
        alpha = 1.0 - 17.0 * m / 15.0 - beta;
    }
    if alpha < beta {
        return Err(error("@opFillet: curvature fillets of so great a magnitude aren't supported yet"));
    }
    let at = |s: f64, t: f64| add(c, add(scale(sub(a, c), s), scale(sub(b, c), t)));
    let poles = [
        at(1.0, 0.0),
        at(1.0 - 0.3 * m, 0.0),
        at(1.0 - 0.6 * m, 0.0),
        at(alpha, beta),
        at(beta, alpha),
        at(0.0, 1.0 - 0.6 * m),
        at(0.0, 1.0 - 0.3 * m),
        at(0.0, 1.0),
    ];
    let knots = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
    occt::bspline_edge(&poles, &[], &knots, 5, false).map_err(crate::occt_error)
}

/// Of the two faces next to an edge, the one whose normal (at the edge's middle) leans most along x, then y, then
/// z. Onshape runs a fillet cylinder's axis along the edge as it runs in this face's loop (batch 047) and measures
/// an asymmetric fillet's `radius` from it at a convex edge (batch 052).
fn leading_face(body: &Shape, edge: &Shape) -> Option<Shape> {
    let m = edge.edge_tangent(0.5, false).ok()?.origin;
    let normal_of = |f: &Shape| -> Option<occt::Vec3> {
        let (u, v) = f.face_fraction(m).ok()?;
        f.face_tangent_plane(u, v).ok().map(|p| p.normal)
    };
    let key = |n: occt::Vec3| [n.x.abs(), n.y.abs(), n.z.abs()];
    body.ancestors(edge, ShapeKind::Face)
        .into_iter()
        .filter_map(|f| normal_of(&f).map(|n| (f, n)))
        .max_by(|(_, a), (_, b)| {
            let (ka, kb) = (key(*a), key(*b));
            (0..3)
                .map(|i| if (ka[i] - kb[i]).abs() < 1e-9 { std::cmp::Ordering::Equal } else { ka[i].total_cmp(&kb[i]) })
                .find(|o| *o != std::cmp::Ordering::Equal)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(f, _)| f)
}

/// The face next to an edge that the edge runs forward along: walking along the edge with that face's normal up,
/// the face is on the left. Uneven chamfers measure their first distance on it.
fn left_face(body: &Shape, edge: &Shape) -> Option<Shape> {
    body.ancestors(edge, ShapeKind::Face).into_iter().find(|face| {
        face.sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(edge) && e.orientation() == edge.orientation())
    })
}

pub fn chamfer(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let width = |key: &str| values::length(field(definition, key), key);
    let opposite = values::opt_bool(definition, "oppositeDirection")?.unwrap_or(false);
    let kind = match field(definition, "chamferType").as_str() {
        Some("EQUAL_OFFSETS") | None => occt::Chamfer::Equal(width("width")?),
        Some("TWO_OFFSETS") => {
            let (a, b) = (width("width1")?, width("width2")?);
            if opposite { occt::Chamfer::TwoDistances(b, a) } else { occt::Chamfer::TwoDistances(a, b) }
        }
        Some("OFFSET_ANGLE") => {
            occt::Chamfer::DistanceAngle(width("width")?, values::angle(field(definition, "angle"), "angle")?)
        }
        Some(other) => return Err(error(format!("@opChamfer: {other} isn't supported yet"))),
    };
    let groups = blend_edges(model, definition)?;
    if groups.is_empty() {
        return Err(regen_error("CHAMFER_SELECT_EDGES"));
    }
    let mut results = Vec::new();
    for (b, edges) in groups {
        let body = model.bodies[b].shape.clone();
        let mut faces = Vec::new();
        for edge in &edges {
            let mut face = left_face(&body, edge).ok_or_else(|| regen_error("CHAMFER_FAILED"))?;
            if opposite && matches!(kind, occt::Chamfer::DistanceAngle(..)) {
                face = body
                    .ancestors(edge, ShapeKind::Face)
                    .into_iter()
                    .find(|f| !f.is_same(&face))
                    .ok_or_else(|| regen_error("CHAMFER_FAILED"))?;
            }
            faces.push(face);
        }
        let op = occt::chamfer(&body, &edges, &faces, kind).map_err(|_| regen_error("CHAMFER_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

/// `@opModifyFillet`: removes fillets (the faces are deleted and their neighbours extended to meet), or changes
/// their radius (removed, then the edge where their tangent neighbours now meet is filleted again).
pub fn modify_fillet(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let change = match field(definition, "modifyFilletType").as_str() {
        Some("REMOVE_FILLET") => None,
        Some("CHANGE_RADIUS") => Some(values::length(field(definition, "radius"), "opModifyFillet radius")?),
        other => return Err(error(format!("@opModifyFillet: modifyFilletType {other:?} isn't supported"))),
    };
    let refillet = change.is_some() && values::opt_bool(definition, "reFillet")?.unwrap_or(false);
    let fillets = query::evaluate(model, field(definition, "faces"))?;
    let fillets: Vec<EntityRef> = fillets.into_iter().filter(|r| r.kind == EntityType::Face).collect();
    if fillets.is_empty() {
        return Err(regen_error("MODIFY_FILLET_SELECT_FACES"));
    }
    // Refilleting, the fillets next to these (curved faces tangent to them, and on through tangent curved faces)
    // are taken away too and made again afterwards with their own radii (batch 028).
    let neighbours: Vec<EntityRef> = if refillet { tangent_curved_neighbours(model, &fillets) } else { Vec::new() };
    let all: Vec<EntityRef> = fillets.iter().chain(neighbours.iter()).copied().collect();
    let rails_of = |fillet: EntityRef| -> Vec<Tid> {
        let face = model.shape(fillet);
        let body = model.body(fillet);
        let mut found = Vec::new();
        for edge in face.visible_sub_shapes(ShapeKind::Edge) {
            for (index, other) in body.faces.iter().enumerate() {
                let r = EntityRef { body: fillet.body, kind: EntityType::Face, index };
                if all.contains(&r) || other.shape.is_same(face) {
                    continue;
                }
                let shares = other.shape.visible_sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(&edge));
                if shares && edge.faces_tangent_along(face, &other.shape, 1e-6) && !found.contains(&other.tid) {
                    found.push(other.tid);
                }
            }
        }
        found
    };
    // For each fillet, the transient ids of the two faces it is tangent to.
    let mut rails: Vec<(Tid, Tid)> = Vec::new();
    for &fillet in &fillets {
        if let [a, b] = rails_of(fillet)[..] {
            rails.push((a, b));
        } else if change.is_some() {
            return Err(regen_error("MODIFY_FILLET_FAILED"));
        }
    }
    // The neighbours made again: the cylinders among them, by their rails and radius (their corners come back as
    // the fillets meet).
    let mut again: Vec<(Tid, Tid, f64)> = Vec::new();
    for &n in &neighbours {
        let Ok(surface) = model.shape(n).surface_definition() else { continue };
        if surface.kind != 1 {
            continue;
        }
        if let [a, b] = rails_of(n)[..]
            && !again.iter().any(|(x, y, _)| (*x, *y) == (a, b) || (*x, *y) == (b, a))
        {
            again.push((a, b, surface.radius1));
        }
    }
    let mut results = Vec::new();
    let mut groups: Vec<(usize, Vec<Shape>)> = Vec::new();
    for r in &all {
        let shape = model.shape(*r).clone();
        match groups.iter_mut().find(|(b, _)| *b == r.body) {
            Some((_, list)) => list.push(shape),
            None => groups.push((r.body, vec![shape])),
        }
    }
    for (b, faces) in groups {
        let op = occt::delete_faces(&model.bodies[b].shape, &faces).map_err(|_| regen_error("MODIFY_FILLET_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    let Some(radius) = change else { return Ok(Value::UNDEFINED) };
    refillet_rails(model, &id, rails.iter().map(|&(a, b)| (a, b, radius)).collect())?;
    let mut radii: Vec<f64> = Vec::new();
    for &(_, _, r) in &again {
        if !radii.iter().any(|x| (x - r).abs() < 1e-12) {
            radii.push(r);
        }
    }
    for r in radii {
        refillet_rails(model, &id, again.iter().filter(|g| (g.2 - r).abs() < 1e-12).copied().collect())?;
    }
    Ok(Value::UNDEFINED)
}

/// The curved faces tangent to these fillets, and on through curved faces tangent to those.
fn tangent_curved_neighbours(model: &Model, fillets: &[EntityRef]) -> Vec<EntityRef> {
    let mut found: Vec<EntityRef> = Vec::new();
    let mut queue: Vec<EntityRef> = fillets.to_vec();
    while let Some(r) = queue.pop() {
        let face = model.shape(r).clone();
        let body = model.body(r).clone();
        for edge in face.visible_sub_shapes(ShapeKind::Edge) {
            for (index, other) in body.faces.iter().enumerate() {
                let o = EntityRef { body: r.body, kind: EntityType::Face, index };
                if fillets.contains(&o) || found.contains(&o) || other.shape.is_same(&face) {
                    continue;
                }
                if other.shape.face_plane().is_ok() {
                    continue;
                }
                let shares = other.shape.visible_sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(&edge));
                if shares && edge.faces_tangent_along(&face, &other.shape, 1e-6) {
                    found.push(o);
                    queue.push(o);
                }
            }
        }
    }
    found
}

/// Fillets the edges where each pair of faces (by transient id, through what became of them) now meets.
fn refillet_rails(model: &mut Model, id: &FeatureId, rails: Vec<(Tid, Tid, f64)>) -> R<()> {
    let image = |model: &Model, tid: Tid| -> Option<EntityRef> {
        model.everything(Some(EntityType::Face)).into_iter().find(|r| {
            let entity = model.entity(*r).expect("a face");
            entity.tid == tid || entity.ancestry.identity.contains(&tid)
        })
    };
    let mut by_body: Vec<(usize, Vec<Shape>, f64)> = Vec::new();
    for (a, b, radius) in rails {
        let (Some(a), Some(b)) = (image(model, a), image(model, b)) else {
            return Err(regen_error("MODIFY_FILLET_FAILED"));
        };
        let theirs = model.shape(b).visible_sub_shapes(ShapeKind::Edge);
        for edge in model.shape(a).visible_sub_shapes(ShapeKind::Edge) {
            if theirs.iter().any(|e| e.is_same(&edge)) {
                match by_body.iter_mut().find(|(g, _, _)| *g == a.body) {
                    Some((_, list, _)) => list.push(edge),
                    None => by_body.push((a.body, vec![edge], radius)),
                }
            }
        }
    }
    let mut results: Vec<(usize, occt::Operation)> = Vec::new();
    for (b, edges, radius) in by_body {
        let op =
            occt::fillet(&model.bodies[b].shape, &edges, radius).map_err(|_| regen_error("MODIFY_FILLET_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, id, results);
    Ok(())
}

/// `@opFullRoundFillet`: rounds the centre faces between two parallel planar side faces into a half cylinder
/// tangent to both.
pub fn full_round(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let one = |key: &str| -> R<Option<EntityRef>> {
        Ok(query::evaluate(model, field(definition, key))?.into_iter().find(|r| r.kind == EntityType::Face))
    };
    let (Some(side1), Some(side2)) = (one("side1Face")?, one("side2Face")?) else {
        return Err(regen_error("FULL_ROUND_FILLET_SELECT_FACES"));
    };
    let centre: Vec<Shape> = query::evaluate(model, field(definition, "centerFaces"))?
        .into_iter()
        .filter(|r| r.kind == EntityType::Face && r.body == side1.body)
        .map(|r| model.shape(r).clone())
        .collect();
    let op = occt::full_round(&model.bodies[side1.body].shape, model.shape(side1), model.shape(side2), &centre)
        .map_err(|_| regen_error("FULL_ROUND_FILLET_FAILED"))?;
    replace_bodies(model, &id, vec![(side1.body, op)]);
    Ok(Value::UNDEFINED)
}

/// `@opFaceBlend`: a rolling ball blend of constant radius between two sets of faces meeting along edges, trimming
/// them to it; the sides' sheets become one. Each side's normal, flipped if asked, points towards the blend.
pub fn face_blend(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    if field(definition, "crossSection").as_str().is_some_and(|c| c != "ROLLING_BALL") {
        return Err(error("@opFaceBlend: only rolling ball blends are supported yet"));
    }
    let plain = field(definition, "blendControlType").as_str().is_none_or(|c| c == "RADIUS")
        && !values::opt_bool(definition, "asymmetric")?.unwrap_or(false)
        && field(definition, "crossSectionShape").as_str().is_none_or(|c| c == "CIRCULAR")
        && field(definition, "trim").as_str().is_none_or(|t| t == "WALLS")
        && !values::opt_bool(definition, "detach")?.unwrap_or(false);
    if !plain {
        return planar_face_blend(model, id, definition);
    }
    let radius = values::length(field(definition, "radius"), "opFaceBlend radius")?;
    let side = |key: &str| -> R<Vec<EntityRef>> {
        Ok(query::evaluate(model, field(definition, key))?.into_iter().filter(|r| r.kind == EntityType::Face).collect())
    };
    let (side1, side2) = (side("side1")?, side("side2")?);
    if side1.is_empty() || side2.is_empty() {
        return Err(regen_error("FACE_BLEND_SELECT_FACES"));
    }
    let shapes = |refs: &[EntityRef]| -> Vec<Shape> { refs.iter().map(|r| model.shape(*r).clone()).collect() };
    let reverse1 = !values::opt_bool(definition, "flipSide1Normal")?.unwrap_or(false);
    let reverse2 = !values::opt_bool(definition, "flipSide2Normal")?.unwrap_or(false);
    let Ok(op) = occt::face_blend(&shapes(&side1), reverse1, &shapes(&side2), reverse2, radius) else {
        // Sheets apart: OCCT only rounds an edge they share.
        return planar_face_blend(model, id, definition);
    };
    // The sides' bodies give way to the blended sheet, which keeps the first's identity.
    let first = model.bodies[side1[0].body].clone();
    let mut bodies: Vec<usize> = side1.iter().chain(&side2).map(|r| r.body).collect();
    bodies.sort_unstable();
    bodies.dedup();
    let info = crate::model::Info::new(id);
    let sheets = occt::connected_shells(&op.shape()).map_err(crate::occt_error)?;
    model.remove_bodies(&bodies);
    for (i, sheet) in sheets.into_iter().enumerate() {
        let tid = (i == 0).then_some(first.tid);
        let new = crate::model::NewBody {
            tid,
            body_type: crate::model::BodyType::Sheet,
            ..crate::model::NewBody::replacing(&first, sheet)
        };
        model.add_body(new, &crate::model::Origins::default(), &info, &|_| None);
    }
    Ok(Value::UNDEFINED)
}

/// A face blend between two planar sides, meeting or not: the section runs round the line where their planes meet,
/// from a point set back along each (as a fillet's, [`section_fillet`]: a radius, two radii — the centre `radius`
/// from side 1 and `secondRadius` from side 2 — or a width, the chord between the two points, `widthRatio` the
/// ratio of the distances from side 1 and side 2 that set them back as two radii would), swept along that line
/// over both sides (untrimmed, further). Each side is cut back to where the blend meets it and all are sewn into
/// one sheet; detached, the blend is a sheet of its own and the sides stay as they are.
fn planar_face_blend(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    use crate::values::{add, cross, dot, normalize, scale, sub};
    let failed = || regen_error("FACE_BLEND_FAILED");
    let side = |key: &str| -> R<Vec<EntityRef>> {
        Ok(query::evaluate(model, field(definition, key))?.into_iter().filter(|r| r.kind == EntityType::Face).collect())
    };
    let (side1, side2) = (side("side1")?, side("side2")?);
    if side1.is_empty() || side2.is_empty() {
        return Err(regen_error("FACE_BLEND_SELECT_FACES"));
    }
    let ([f1], [f2]) = (&side1[..], &side2[..]) else {
        return Err(error("@opFaceBlend: blends other than between two single faces aren't supported yet"));
    };
    let (face1, face2) = (model.shape(*f1).clone(), model.shape(*f2).clone());
    let not_planar = || error("@opFaceBlend: blends other than between planar faces aren't supported yet");
    let (plane1, plane2) =
        (face1.face_plane().map_err(|_| not_planar())?, face2.face_plane().map_err(|_| not_planar())?);
    // Where the planes meet: along `t`, through `c`.
    let t = normalize(cross(plane1.normal, plane2.normal)).ok_or_else(failed)?;
    let (n1, n2) = (plane1.normal, plane2.normal);
    let (h1, h2) = (dot(n1, plane1.origin), dot(n2, plane2.origin));
    let k = dot(n1, n2);
    let det = 1.0 - k * k;
    let c = add(scale(n1, (h1 - k * h2) / det), scale(n2, (h2 - k * h1) / det));
    // Into each side from that line, square to it.
    let into = |plane: &occt::Frame, face: &Shape| -> R<occt::Vec3> {
        let u = normalize(cross(plane.normal, t)).ok_or_else(failed)?;
        let inner = face.face_inner_point().map_err(|_| failed())?;
        Ok(if dot(sub(inner, c), u) >= 0.0 { u } else { scale(u, -1.0) })
    };
    let (u1, u2) = (into(&plane1, &face1)?, into(&plane2, &face2)?);
    // Each side's sense (its normal, flipped if asked) must point to the side of it the blend is on, where the other
    // side goes (batch 054: Onshape refuses sheets apart whose flips std works out wrongly).
    let flipped = |key: &str| -> R<bool> { Ok(values::opt_bool(definition, key)?.unwrap_or(false)) };
    let sense = |n: occt::Vec3, flip: bool| if flip { scale(n, -1.0) } else { n };
    if dot(sense(n1, flipped("flipSide1Normal")?), u2) < 0.0 {
        return Err(regen_error("FACE_BLEND_WRONG_LEFT_WALL_SENSE"));
    }
    if dot(sense(n2, flipped("flipSide2Normal")?), u1) < 0.0 {
        return Err(regen_error("FACE_BLEND_WRONG_RIGHT_WALL_SENSE"));
    }
    let angle = dot(u1, u2).clamp(-1.0, 1.0).acos();
    let asymmetric = values::opt_bool(definition, "asymmetric")?.unwrap_or(false);
    let (s1, s2) = if field(definition, "blendControlType").as_str() == Some("WIDTH") {
        let width = values::length(field(definition, "width"), "opFaceBlend width")?;
        let ratio = if asymmetric { field(definition, "widthRatio").as_number().unwrap_or(1.0) } else { 1.0 };
        // The ratio is of the section's distances from side 1 and side 2, as the two radii set them (batch 056
        // c012: at 60°, ratio 2 sets the sides back 5.2372 and 6.5465 mm; at 120° it leaves side 1 none, which
        // Onshape refuses, 055 c010), scaled so the chord between the two points is the width.
        let (b1, b2) = ((1.0 + ratio * angle.cos()) / angle.sin(), (ratio + angle.cos()) / angle.sin());
        let chord = (b1 * b1 + b2 * b2 - 2.0 * b1 * b2 * angle.cos()).sqrt();
        (width * b1 / chord, width * b2 / chord)
    } else {
        let radius = values::length(field(definition, "radius"), "opFaceBlend radius")?;
        let second = if asymmetric {
            values::length(field(definition, "secondRadius"), "opFaceBlend secondRadius")?
        } else {
            radius
        };
        ((second + radius * angle.cos()) / angle.sin(), (radius + second * angle.cos()) / angle.sin())
    };
    if !(s1 > 1e-9 && s2 > 1e-9) {
        return Err(failed());
    }
    let section = match field(definition, "crossSectionShape").as_str() {
        Some("CONIC") => Section::Conic(field(definition, "rho").as_number().ok_or_else(failed)?),
        Some("CURVATURE") => Section::Curvature(field(definition, "magnitude").as_number().ok_or_else(failed)?),
        _ => Section::Circular,
    };
    // Along the line: over both sides (a long trim), or where they overlap (short, batch 055 c005).
    let extent = |face: &Shape| -> (f64, f64) {
        face.sub_shapes(ShapeKind::Vertex)
            .iter()
            .filter_map(|v| v.vertex_point().ok())
            .map(|p| dot(sub(p, c), t))
            .fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(x), b.max(x)))
    };
    let ((lo1, hi1), (lo2, hi2)) = (extent(&face1), extent(&face2));
    let trim = field(definition, "trim").as_str().unwrap_or("WALLS").to_string();
    let detach = values::opt_bool(definition, "detach")?.unwrap_or(false);
    let short = trim == "SHORT";
    let (lo, hi) = if short { (lo1.max(lo2), hi1.min(hi2)) } else { (lo1.min(lo2), hi1.max(hi2)) };
    if hi - lo < 1e-9 {
        return Err(failed());
    }
    // Untrimmed, the blend runs on past the sides' ends by a tenth of their length at each (batch 054 c007, 055
    // c001–c003: 3 mm of 30, 6 of 60, 5 of sides spanning 50 between them, whatever the radius).
    let (lo, hi) = if trim == "NO_TRIM" { (lo - 0.1 * (hi - lo), hi + 0.1 * (hi - lo)) } else { (lo, hi) };
    let corner = add(c, scale(t, lo));
    let (p1, p2) = (add(corner, scale(u1, s1)), add(corner, scale(u2, s2)));
    // How fast the section runs at its ends (p2's, p1's), in its own parameter.
    let (speed2, speed1) = match section {
        Section::Circular if (s1 - s2).abs() < 1e-12 => (s1 * (angle / 2.0).tan(), s1 * (angle / 2.0).tan()),
        Section::Circular => (2.0 * (angle / 2.0).sin() * s2, 2.0 * (angle / 2.0).sin() * s1),
        Section::Conic(rho) => (2.0 * rho / (1.0 - rho) * s2, 2.0 * rho / (1.0 - rho) * s1),
        Section::Curvature(m) => (3.0 * m * s2, 3.0 * m * s1),
    };
    let curve = match section {
        Section::Circular if (s1 - s2).abs() < 1e-12 => {
            let bisector = normalize(add(u1, u2)).ok_or_else(failed)?;
            let radius = s1 * (angle / 2.0).tan();
            let centre = add(corner, scale(bisector, radius / (angle / 2.0).sin()));
            occt::arc_through(p2, sub(centre, scale(bisector, radius)), p1).map_err(crate::occt_error)?
        }
        // Onshape's face blends keep their conic sections exact (its areas are the exact conics', batch 054), unlike
        // its fillets' cubic ones.
        Section::Circular => conic_edge(p2, corner, p1, (angle / 2.0).sin())?,
        Section::Conic(rho) => conic_edge(p2, corner, p1, rho / (1.0 - rho))?,
        Section::Curvature(m) => curvature_section(p2, corner, p1, m)?,
    };
    let swept = occt::prism(&curve, scale(t, hi - lo)).map_err(crate::occt_error)?.shape();
    let mut blend_faces = swept.sub_shapes(ShapeKind::Face);
    // Trimmed to walls of unequal extent, the blend runs along each wall as far as that wall does, and its ends
    // slant across from one wall's corner to the other's: in the blend's parameters, a cubic leaving each corner
    // along the section, as fast as the chord between the corners is long (batch 055 c004, a circular blend OCCT
    // rounds; built this way instead, it matches Onshape's area and edges too).
    if trim == "WALLS" && !detach && ((lo1 - lo2).abs() > 1e-9 || (hi1 - hi2).abs() > 1e-9) {
        let face = blend_faces.first().ok_or_else(failed)?.clone();
        let ((fu0, fv0), (fu1, fv1)) = face.face_uv_bounds().map_err(|_| failed())?;
        let (at_p2, start) = face.face_fraction(p2).map_err(|_| failed())?;
        let (_, finish) = face.face_fraction(add(p2, scale(t, hi - lo))).map_err(|_| failed())?;
        // The section's parameter at side 2's end and at side 1's; the sweep's at a point along the line.
        let (ua, ub) = if at_p2 < 0.5 { (fu0, fu1) } else { (fu1, fu0) };
        let toward = if ub > ua { 1.0 } else { -1.0 };
        let v_at = |x: f64| fv0 + (start + (x - lo) / (hi - lo) * (finish - start)) * (fv1 - fv0);
        let end = |x1: f64, x2: f64| -> Vec<(f64, f64)> {
            let chord = values::norm(sub(add(p1, scale(t, x1 - lo)), add(p2, scale(t, x2 - lo))));
            // Each handle reaches at most across the whole section (batch 056 c009: 1.637 held at 1).
            let handle = |speed: f64| (chord / (3.0 * speed)).min((ub - ua).abs());
            let poles = [
                (ub, v_at(x1)),
                (ub - toward * handle(speed1), v_at(x1)),
                (ua + toward * handle(speed2), v_at(x2)),
                (ua, v_at(x2)),
            ];
            (0..=32)
                .map(|k| {
                    let s = k as f64 / 32.0;
                    let b = [(1.0 - s).powi(3), 3.0 * (1.0 - s).powi(2) * s, 3.0 * (1.0 - s) * s * s, s.powi(3)];
                    (0..4).fold((0.0, 0.0), |(u, v), i| (u + b[i] * poles[i].0, v + b[i] * poles[i].1))
                })
                .collect()
        };
        let mut low = end(lo1, lo2);
        low.reverse();
        let boundary =
            vec![vec![(ub, v_at(lo1)), (ub, v_at(hi1))], end(hi1, hi2), vec![(ua, v_at(hi2)), (ua, v_at(lo2))], low];
        blend_faces = vec![occt::face_from_uv(&face, &boundary).map_err(|_| failed())?];
    }
    // Each side cut back to where the blend meets it: what lies at least its setback from the line, or with a
    // short trim, notched only where the blend runs.
    let trimmed = |face: &Shape, u: occt::Vec3, s: f64| -> R<Vec<Shape>> {
        let big = 10.0 * (hi - lo + s1 + s2 + 1.0);
        let kept = if short {
            let n = cross(u, t);
            let frame = occt::Frame { origin: sub(add(c, scale(t, lo)), scale(n, big)), normal: n, x: t };
            let y = cross(n, t);
            let (y0, y1) = if dot(y, u) > 0.0 { (-big, s) } else { (-s, big) };
            let slab = occt::plane_face(frame, 0.0, hi - lo, y0, y1).map_err(crate::occt_error)?;
            let notch = occt::prism(&slab, scale(n, 2.0 * big)).map_err(crate::occt_error)?.shape();
            occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(face), &[notch], true)
        } else {
            let frame = occt::Frame { origin: add(c, scale(u, s)), normal: u, x: t };
            let slab = occt::plane_face(frame, -big, big, -big, big).map_err(crate::occt_error)?;
            let half = occt::prism(&slab, scale(u, big)).map_err(crate::occt_error)?.shape();
            occt::boolean(occt::BooleanKind::Intersection, std::slice::from_ref(face), &[half], true)
        };
        Ok(kept.map_err(|_| failed())?.shape().sub_shapes(ShapeKind::Face))
    };
    let info = crate::model::Info::new(id);
    if !detach {
        let mut faces = trimmed(&face1, u1, s1)?;
        faces.extend(trimmed(&face2, u2, s2)?);
        faces.extend(blend_faces);
        let sewn = occt::sew_faces(&faces, 1e-6).map_err(|_| failed())?;
        let first = model.bodies[f1.body].clone();
        let mut bodies = vec![f1.body, f2.body];
        bodies.sort_unstable();
        bodies.dedup();
        model.remove_bodies(&bodies);
        let sheets = occt::connected_shells(&sewn).map_err(crate::occt_error)?;
        for (i, sheet) in sheets.into_iter().enumerate() {
            let new = crate::model::NewBody {
                tid: (i == 0).then_some(first.tid),
                body_type: crate::model::BodyType::Sheet,
                ..crate::model::NewBody::replacing(&first, sheet)
            };
            model.add_body(new, &crate::model::Origins::default(), &info, &|_| None);
        }
    } else {
        let sheet = occt::sew_faces(&blend_faces, 1e-6).map_err(|_| failed())?;
        super::add_simple(model, sheet, crate::model::BodyType::Sheet, &info, false);
    }
    Ok(Value::UNDEFINED)
}

/// The rational quadratic from `p0` to `p2` with its middle pole `p1` weighted `w`: an exact conic.
fn conic_edge(p0: occt::Vec3, p1: occt::Vec3, p2: occt::Vec3, w: f64) -> R<Shape> {
    occt::bspline_edge(&[p0, p1, p2], &[1.0, w, 1.0], &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0], 2, false)
        .map_err(crate::occt_error)
}

/// A variable fillet: the radius at each end of each edge is set at its vertex (`vertexSettings`), the fillet's
/// radius elsewhere, and goes from one to the other smoothly (level at both ends) or linearly.
fn variable_fillet(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    if field(definition, "pointOnEdgeSettings").as_array().is_some_and(|a| !a.is_empty()) {
        return Err(error("@opFillet: radii at points on edges aren't supported yet"));
    }
    let radius = values::length(field(definition, "radius"), "opFillet radius")?;
    let smooth = values::opt_bool(definition, "smoothTransition")?.unwrap_or(false);
    let mut at_vertex: Vec<(occt::Vec3, f64)> = Vec::new();
    for setting in field(definition, "vertexSettings").as_array().map(|a| a.to_vec()).unwrap_or_default() {
        let r = values::length(field(&setting, "vertexRadius"), "opFillet vertexRadius")?;
        for v in query::evaluate(model, field(&setting, "vertex"))? {
            if let Ok(p) = model.shape(v).vertex_point() {
                at_vertex.push((p, r));
            }
        }
    }
    let radius_at = |p: occt::Vec3| {
        at_vertex.iter().find(|(q, _)| values::norm(values::sub(*q, p)) < 1e-9).map(|(_, r)| *r).unwrap_or(radius)
    };
    let groups = blend_edges(model, definition)?;
    if groups.is_empty() {
        return Err(regen_error("FILLET_SELECT_EDGES"));
    }
    let mut results = Vec::new();
    for (b, edges) in groups {
        let (mut p0, mut r0, mut r1) = (Vec::new(), Vec::new(), Vec::new());
        for edge in &edges {
            let ends = edge.edge_end_vertices().unwrap_or_default();
            let points: Vec<occt::Vec3> = ends.iter().filter_map(|v| v.vertex_point().ok()).collect();
            let (a, z) = match points.as_slice() {
                [a, z, ..] => (*a, *z),
                [a] => (*a, *a),
                [] => return Err(regen_error("FILLET_FAILED")),
            };
            p0.push(a);
            r0.push(radius_at(a));
            r1.push(radius_at(z));
        }
        let op = occt::variable_fillet(&model.bodies[b].shape, &edges, &p0, &r0, &r1, smooth)
            .map_err(|_| regen_error("FILLET_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

/// A partial fillet: what a full fillet would remove, only where a bound keeps it. Each bound is a plane square to
/// its edge at a parameter along it. The fillet ends square.
fn partial_fillet(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let radius = values::length(field(definition, "radius"), "opFillet radius")?;
    let mut planes: Vec<occt::Frame> = Vec::new();
    for bound in field(definition, "partialFilletBounds").as_array().map(|a| a.to_vec()).unwrap_or_default() {
        let edges = query::evaluate(model, field(&bound, "boundaryEdge"))?;
        let edge = edges.iter().find(|r| r.kind == EntityType::Edge).ok_or_else(|| regen_error("FILLET_FAILED"))?;
        let t = field(&bound, "boundaryParameter").as_number().ok_or_else(|| error("@opFillet: boundaryParameter"))?;
        let flipped = field(&bound, "isFlipped").as_bool() == Some(true);
        let at = model.shape(*edge).edge_tangent(t, false).map_err(|_| regen_error("FILLET_FAILED"))?;
        let normal = if flipped { values::scale(at.direction, -1.0) } else { at.direction };
        planes.push(occt::Frame { origin: at.origin, normal, x: values::canonical_x(normal) });
    }
    let groups = blend_edges(model, definition)?;
    if groups.is_empty() {
        return Err(regen_error("FILLET_SELECT_EDGES"));
    }
    let mut results = Vec::new();
    for (b, edges) in groups {
        let body = model.bodies[b].shape.clone();
        let full = occt::fillet(&body, &edges, radius).map_err(|_| regen_error("FILLET_FAILED"))?;
        let removed =
            occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(&body), &[full.shape()], false)
                .map_err(|_| regen_error("FILLET_FAILED"))?
                .shape();
        // Each bound keeps the removed material on the side its edge comes from (flipped, the side it goes to);
        // the fillet is where any bound keeps it. (Batch 010, c003: bounds at a quarter and three quarters fillet
        // the two outer quarters.)
        let bounds = body.bounding_box(false).map_err(crate::occt_error)?;
        let size = values::norm(values::sub(bounds.max, bounds.min)) * 4.0 + 1.0;
        let removed = removed.sub_shapes(ShapeKind::Solid);
        let mut pieces = Vec::new();
        for plane in &planes {
            let cutter = occt::plane_face(*plane, -size, size, -size, size).map_err(crate::occt_error)?;
            for piece in &removed {
                let split = occt::split(std::slice::from_ref(piece), std::slice::from_ref(&cutter))
                    .map_err(crate::occt_error)?;
                for part in split.shape().sub_shapes(ShapeKind::Solid) {
                    let centre = part.centroid().map_err(crate::occt_error)?;
                    if values::dot(values::sub(centre, plane.origin), plane.normal) < 0.0 {
                        pieces.push(part);
                    }
                }
            }
        }
        if pieces.is_empty() {
            return Err(regen_error("FILLET_FAILED"));
        }
        let op = occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(&body), &pieces, true)
            .map_err(|_| regen_error("FILLET_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}
