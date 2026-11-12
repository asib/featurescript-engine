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
    for (b, edges) in &mut groups {
        // A closed edge OCCT split at a seam vertex is shown as its first piece: the others go with it (batch 124
        // c004: a spherical pocket's rim).
        let pieces = model.bodies[*b].shape.hidden_edge_pieces();
        let hidden: Vec<Shape> = edges
            .iter()
            .flat_map(|e| pieces.iter().filter(|(shown, _)| shown.is_same(e)).map(|(_, h)| h.clone()))
            .collect();
        edges.extend(hidden);
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
        Some("CONIC") => return variable_section_fillet(model, id, definition, Section::Conic(field(definition, "rho").as_number().unwrap_or(0.5))),
        Some("CURVATURE") => {
            let m = field(definition, "magnitude").as_number().unwrap_or(0.5);
            return variable_section_fillet(model, id, definition, Section::Curvature(m));
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
    if values::opt_bool(definition, "isAsymmetric")?.unwrap_or(false) && values::opt_bool(definition, "isVariable")?.unwrap_or(false) {
        return variable_section_fillet(model, id, definition, Section::Circular);
    }
    if asymmetric {
        return Err(error("@opFillet: isAsymmetric isn't supported yet"));
    }
    // Only the fillet's surface, as sheets, the bodies left as they are (std measures the angle of a bend between a
    // flat and a curved wall of a sheet metal definition so, batch 069).
    if values::opt_bool(definition, "createDetachedSurface")?.unwrap_or(false) {
        let radius = values::length(field(definition, "radius"), "opFillet radius")?;
        let groups = blend_edges(model, definition)?;
        if groups.is_empty() {
            return Err(regen_error("FILLET_SELECT_EDGES"));
        }
        let info = crate::model::Info::new(id.clone());
        for (b, edges) in groups {
            let body = model.bodies[b].shape.clone();
            let op = occt::fillet(&body, &edges, radius).map_err(|_| regen_error("FILLET_FAILED"))?;
            for edge in &edges {
                for face in op.generated(edge) {
                    model.add_body(
                        crate::model::NewBody::new(face, crate::model::BodyType::Sheet, info.clone()),
                        &crate::model::Origins::default(),
                        &info,
                        &|_| None,
                    );
                }
            }
        }
        return Ok(Value::UNDEFINED);
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
        // Circles between a plane and a sphere about their axis: a torus revolved round it (OCCT's fillet breaks
        // where the sphere's seam meets the circle, batch 124 c004).
        if let Some(op) = sphere_rim_blend(&body, &edges, RimBlend::Fillet(radius))? {
            results.push((b, op));
            continue;
        }
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
    super::sense_from_older_faces(model, &id);
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
            // Along an arc between a plane and a cylinder about it, the variable fillet's wedge with the setbacks
            // held (batch 082 c001–c003).
            if edge.curve_type().ok() == Some(occt::CurveType::Circle) {
                let mut faces = body.ancestors(edge, ShapeKind::Face);
                if faces.len() != 2 {
                    return Err(failed());
                }
                if leading_face(&body, edge).is_some_and(|l| l.is_same(&faces[1])) {
                    faces.swap(0, 1);
                }
                let (_, convex) =
                    arc_variable_wedge(&body, edge, [&faces[0], &faces[1]], [[radius; 4], [radius; 4]], section)?;
                let (d1, d2) = if convex { (radius, other) } else { (other, radius) };
                let (wedge, inside) =
                    arc_variable_wedge(&body, edge, [&faces[0], &faces[1]], [[d1; 4], [d2; 4]], section)?;
                if inside {
                    cut.push((wedge.clone(), wedge))
                } else {
                    fill.push((wedge.clone(), wedge))
                }
                continue;
            }
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
        // curvature fillet of magnitude 0.82), or fail: then with the extrusions themselves. A fillet that changes
        // nothing either way, or leaves an invalid part, fails: a curvature section within micrometres of its corner
        // (batches 113 c009, 114 c019, magnitude 0.99875) leaves a tool the booleans can't cut with.
        let apply = |kind: occt::BooleanKind, shape: &Shape, tools: &[(Shape, Shape)]| -> R<occt::Operation> {
            let before = shape.volume().unwrap_or(0.0);
            let changed = |o: &occt::Operation| {
                let result = o.shape();
                (result.volume().unwrap_or(0.0) - before).abs() > 1e-15 && result.is_valid()
            };
            let converted: Vec<Shape> = tools.iter().map(|(c, _)| c.clone()).collect();
            if let Ok(o) = occt::boolean(kind, std::slice::from_ref(shape), &converted, true)
                && changed(&o)
            {
                return Ok(o);
            }
            let raw: Vec<Shape> = tools.iter().map(|(_, r)| r.clone()).collect();
            match occt::boolean(kind, std::slice::from_ref(shape), &raw, true) {
                Ok(o) if changed(&o) => Ok(o),
                _ => Err(failed()),
            }
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

/// How a sphere's rim is blended: a circular fillet of this radius, or an equal chamfer this far from the edge
/// along the plane and in a straight line on the sphere (batch 125 c005).
#[derive(Clone, Copy)]
enum RimBlend {
    Fillet(f64),
    Chamfer(f64),
}

/// A fillet or chamfer of circles each between two faces that are planes square to the circle's axis or spheres
/// centred on it: in the half plane through the axis, each face is a line or a circle, and the fillet's arc is
/// tangent to both on the material's side (beyond them at a concave edge), or the chamfer's line runs between the
/// points its width from the edge on each (along a plane, in a straight line on a sphere); its corner region,
/// closed off past the faces (into the void to cut, into the material to fill), is revolved round the axis and cut
/// away or filled. A plane's rim round a spherical pocket or dome (batches 124 c004, 125 c004, c005), two balls
/// united (batch 127 c007). None unless every edge is such a circle.
fn sphere_rim_blend(body: &Shape, edges: &[Shape], blend: RimBlend) -> R<Option<occt::Operation>> {
    use crate::values::{add, cross, dot, norm, normalize, scale, sub};
    let failed = || regen_error("FILLET_FAILED");
    // A face in the half plane: a line h = 0 (a plane through the circle) or a circle about (0, h) of radius r.
    #[derive(Clone, Copy)]
    enum Profile {
        Line,
        Circle(f64, f64),
    }
    let mut cut = Vec::new();
    let mut fill = Vec::new();
    for edge in edges {
        if edge.curve_type().ok() != Some(occt::CurveType::Circle) {
            return Ok(None);
        }
        let faces = body.ancestors(edge, ShapeKind::Face);
        if faces.len() != 2 {
            return Ok(None);
        }
        let circle = edge.curve_definition().map_err(|_| failed())?;
        let (c, rho) = (circle.frame.origin, circle.radius1);
        let n = normalize(circle.frame.normal).ok_or_else(failed)?;
        let e = normalize(circle.frame.x).ok_or_else(failed)?;
        // 2D (r along e, h along n) to 3D.
        let at = |r: f64, h: f64| add(c, add(scale(e, r), scale(n, h)));
        let mut profiles = Vec::new();
        let mut spheres = 0;
        for face in &faces {
            let Ok(d) = face.surface_definition() else { return Ok(None) };
            match d.kind {
                0 if norm(cross(d.frame.normal, n)) < 1e-9 && dot(sub(d.frame.origin, c), n).abs() < 1e-9 => {
                    profiles.push(Profile::Line)
                }
                3 => {
                    let off = sub(d.frame.origin, c);
                    if norm(cross(off, n)) > 1e-9 {
                        return Ok(None);
                    }
                    spheres += 1;
                    profiles.push(Profile::Circle(dot(off, n), d.radius1));
                }
                _ => return Ok(None),
            }
        }
        if spheres == 0 {
            return Ok(None);
        }
        let probe = 1e-4 * rho;
        let on_face = |face: &Shape, (r, h): (f64, f64)| {
            occt::vertex(at(r, h)).and_then(|v| v.distance(face)).is_ok_and(|d| d.distance < 1e-7)
        };
        let inside = |(r, h): (f64, f64)| body.contains_point(at(r, h)).unwrap_or(false);
        // For each face: a step `s` from the edge along it toward the face (a point there), its unit normal at a
        // point (r, h) as seen from its centre or upward, and which side of it holds material.
        struct Side {
            profile: Profile,
            toward: f64,
            material: f64,
        }
        let point_along = |profile: Profile, s: f64, toward: f64| -> (f64, f64) {
            match profile {
                Profile::Line => (rho + toward * s, 0.0),
                Profile::Circle(h0, radius) => {
                    // Turned about the centre by the angle whose chord is `s`.
                    let angle = toward * 2.0 * (s / (2.0 * radius)).min(1.0).asin();
                    let (x, y) = (rho, -h0);
                    (x * angle.cos() - y * angle.sin(), x * angle.sin() + y * angle.cos() + h0)
                }
            }
        };
        let normal_at = |profile: Profile, (r, h): (f64, f64)| -> (f64, f64) {
            match profile {
                Profile::Line => (0.0, 1.0),
                Profile::Circle(h0, _) => {
                    let l = (r * r + (h - h0) * (h - h0)).sqrt();
                    (r / l, (h - h0) / l)
                }
            }
        };
        let mut sides = Vec::new();
        for (face, profile) in faces.iter().zip(&profiles) {
            let toward = [1.0, -1.0]
                .into_iter()
                .find(|&t| on_face(face, point_along(*profile, probe, t)))
                .ok_or_else(failed)?;
            let q = point_along(*profile, probe, toward);
            let nn = normal_at(*profile, q);
            let material = match (
                inside((q.0 + probe * nn.0, q.1 + probe * nn.1)),
                inside((q.0 - probe * nn.0, q.1 - probe * nn.1)),
            ) {
                (true, false) => 1.0,
                (false, true) => -1.0,
                _ => return Ok(None),
            };
            sides.push(Side { profile: *profile, toward, material });
        }
        let edge_2d = (rho, 0.0);
        // Convex where the material is where both faces' material sides meet: on one's material side but off the
        // other's, it's void; at a concave edge, where material is on either side, it isn't.
        let convex = {
            let (n0, n1) = (normal_at(sides[0].profile, edge_2d), normal_at(sides[1].profile, edge_2d));
            let p = (
                rho + sides[0].material * probe * n0.0 - sides[1].material * 2.0 * probe * n1.0,
                sides[0].material * probe * n0.1 - sides[1].material * 2.0 * probe * n1.1,
            );
            !inside(p)
        };
        let tau = if convex { 1.0 } else { -1.0 };
        // The section's feet on the two faces, and a point between for an arc.
        let (foot0, foot1, mid) = match blend {
            RimBlend::Fillet(radius) => {
                // The centre: `radius` off each face on the material's side (beyond at a concave edge).
                let off = |side: &Side| tau * side.material * radius;
                let centre = match (sides[0].profile, sides[1].profile) {
                    (Profile::Line, Profile::Circle(h0, r0)) | (Profile::Circle(h0, r0), Profile::Line) => {
                        let (line, sphere) = if matches!(sides[0].profile, Profile::Line) { (&sides[0], &sides[1]) } else { (&sides[1], &sides[0]) };
                        let qh = off(line);
                        let reach = r0 + off(sphere);
                        let q2 = reach * reach - (qh - h0) * (qh - h0);
                        if q2 <= 0.0 || reach <= 0.0 {
                            return Err(failed());
                        }
                        (q2.sqrt(), qh)
                    }
                    (Profile::Circle(h0, r0), Profile::Circle(h1, r1)) => {
                        let (a, b) = (r0 + off(&sides[0]), r1 + off(&sides[1]));
                        if a <= 0.0 || b <= 0.0 || (h1 - h0).abs() < 1e-12 {
                            return Err(failed());
                        }
                        let h = (a * a - b * b) / (2.0 * (h1 - h0)) + (h0 + h1) / 2.0;
                        let q2 = a * a - (h - h0) * (h - h0);
                        if q2 <= 0.0 {
                            return Err(failed());
                        }
                        (q2.sqrt(), h)
                    }
                    _ => return Ok(None),
                };
                let foot = |side: &Side| match side.profile {
                    Profile::Line => (centre.0, 0.0),
                    Profile::Circle(h0, r0) => {
                        let l = (centre.0 * centre.0 + (centre.1 - h0) * (centre.1 - h0)).sqrt();
                        (r0 * centre.0 / l, h0 + r0 * (centre.1 - h0) / l)
                    }
                };
                let (f0, f1) = (foot(&sides[0]), foot(&sides[1]));
                let (a, b) = ((f0.0 - centre.0) + (f1.0 - centre.0), (f0.1 - centre.1) + (f1.1 - centre.1));
                let l = (a * a + b * b).sqrt();
                (f0, f1, Some((centre.0 + radius * a / l, centre.1 + radius * b / l)))
            }
            RimBlend::Chamfer(width) => {
                for side in &sides {
                    if let Profile::Circle(_, r0) = side.profile
                        && width >= 2.0 * r0
                    {
                        return Err(failed());
                    }
                }
                (point_along(sides[0].profile, width, sides[0].toward), point_along(sides[1].profile, width, sides[1].toward), None)
            }
        };
        let radius = match blend { RimBlend::Fillet(r) | RimBlend::Chamfer(r) => r };
        // Past the faces: into the void to cut (where cutting changes nothing), into the material to fill (where
        // filling changes nothing).
        let delta = 0.5 * radius;
        let away = |side: &Side, (r, h): (f64, f64)| {
            let nn = normal_at(side.profile, (r, h));
            (r - tau * side.material * delta * nn.0, h - tau * side.material * delta * nn.1)
        };
        let out0 = away(&sides[0], foot0);
        let out1 = away(&sides[1], foot1);
        let corner_out = away(&sides[1], away(&sides[0], edge_2d));
        let p = |(r, h): (f64, f64)| at(r, h);
        let line = |a, b| occt::line_edge(p(a), p(b)).map_err(crate::occt_error);
        let section = match mid {
            Some(mid) => occt::arc_through(p(foot0), p(mid), p(foot1)).map_err(crate::occt_error)?,
            None => occt::line_edge(p(foot0), p(foot1)).map_err(crate::occt_error)?,
        };
        let region = occt::fill_edges(&[section, line(foot1, out1)?, line(out1, corner_out)?, line(corner_out, out0)?, line(out0, foot0)?])
            .map_err(|_| failed())?;
        let tool = occt::revolve(&region, occt::Axis { origin: c, direction: n }, std::f64::consts::TAU)
            .map_err(|_| failed())?
            .shape();
        if convex { cut.push(tool) } else { fill.push(tool) }
    }
    let mut shape = body.clone();
    let mut op = None;
    if !cut.is_empty() {
        let o = occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(&shape), &cut, true).map_err(|_| failed())?;
        shape = o.shape();
        op = Some(o);
    }
    if !fill.is_empty() {
        op = Some(occt::boolean(occt::BooleanKind::Union, std::slice::from_ref(&shape), &fill, true).map_err(|_| failed())?);
    }
    Ok(op)
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
/// difference through the shoulder (batches 053, 055, 056, 059: m 0.72 to 0.87, exactly). Past where that α₃ would
/// fall below β₃ (m 0.879), the third pole α₂ moves in too, and α₂ and β₃ both fall linearly with m, α₃ still through
/// the shoulder (`0.125 α₂ + 0.375 (α₃ + β₃) = (1 − m)/2`): fitted to Onshape's poles at m 0.88, 0.9, 0.92, 0.95
/// and 0.97 to 4e-7. That holds to 0.975; from there the sections are Onshape's own, sampled
/// ([`CURVATURE_SAMPLES`]).
fn curvature_section(a: occt::Vec3, c: occt::Vec3, b: occt::Vec3, m: f64) -> R<Shape> {
    use crate::values::{add, scale, sub};
    let at = |(s, t): (f64, f64)| add(c, add(scale(sub(a, c), s), scale(sub(b, c), t)));
    let cos = match (crate::values::normalize(sub(a, c)), crate::values::normalize(sub(b, c))) {
        (Some(x), Some(y)) => crate::values::dot(x, y),
        _ => 0.0,
    };
    let poles: Vec<occt::Vec3> = curvature_coefficients(m, cos)?.into_iter().map(at).collect();
    occt::bspline_edge(&poles, &[], &CURVATURE_KNOTS, 5, false).map_err(crate::occt_error)
}

/// A curvature section's knots: quintic, two spans.
const CURVATURE_KNOTS: [f64; 14] = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];

/// Onshape's curvature sections from magnitude 0.975 on, as (m, α₂, α₃, β₃): past 0.979 they follow no law found (a
/// constrained optimisation, its constraints switching on and off: α₃ = 2(1 − m)/3 from 0.9825 to 0.9875, α₂ = 1 − m
/// from 0.9925, β₃ below nothing at 0.99875), so they are taken from Onshape's own at a right angle, every 0.00125
/// (batches 055 c019, 063 c001–c003, 064 c001–c006, 113 c001–c009), and interpolated between (to within 1.2e-3 mm of
/// Onshape's at every 0.0025 from the samples every 0.0025 before, so about a quarter of that now). Each passes
/// through the shoulder, which interpolation keeps. At 60° and 120° corners the section differs (113 c010, c011:
/// alike, 3e-3 mm off the right angle's at 0.99): [`CURVATURE_SAMPLES_60`].
const CURVATURE_SAMPLES: [(f64, f64, f64, f64); 19] = [
    (0.975, 0.058_756_3, 0.013_675_0, 0.000_073_0),
    (0.9775, 0.048_479_3, 0.013_766_3, 0.000_074_0),
    (0.97875, 0.043_340_7, 0.013_527_5, 0.000_359_0),
    (0.98, 0.038_195_0, 0.013_282_5, 0.000_652_5),
    (0.98125, 0.032_881_5, 0.012_500_0, 0.001_539_5),
    (0.9825, 0.027_748_0, 0.011_666_8, 0.002_417_2),
    (0.98375, 0.023_594_5, 0.010_833_3, 0.002_968_5),
    (0.985, 0.020_652_5, 0.010_000_0, 0.003_115_8),
    (0.98625, 0.018_371_7, 0.009_166_7, 0.003_042_8),
    (0.9875, 0.016_001_5, 0.008_333_3, 0.002_999_5),
    (0.98875, 0.013_310_2, 0.007_489_5, 0.003_073_8),
    (0.99, 0.010_738_8, 0.006_650_8, 0.003_103_0),
    (0.99125, 0.008_828_0, 0.005_865_8, 0.002_858_3),
    (0.9925, 0.007_500_0, 0.005_087_2, 0.002_412_7),
    (0.99375, 0.006_250_0, 0.004_320_7, 0.001_929_3),
    (0.995, 0.005_000_0, 0.003_573_8, 0.001_426_3),
    (0.99625, 0.003_750_0, 0.002_850_5, 0.000_899_5),
    (0.9975, 0.002_500_0, 0.002_132_3, 0.000_367_8),
    (0.99875, 0.001_250_0, 0.001_364_5, -0.000_114_5),
];

/// The same at 60° and 120° corners (batches 114 c001–c019, 113 c010): past 0.975 the section depends on the corner's
/// angle, through cos² (45° and 135° alike, batch 114 c020, c022), and from 0.99375 a 60° corner's is a right angle's.
const CURVATURE_SAMPLES_60: [(f64, f64, f64, f64); 20] = [
    (0.975, 0.062_625_4, 0.012_353_2, 0.000_105_2),
    (0.97625, 0.057_534_6, 0.012_404_9, 0.000_083_7),
    (0.9775, 0.052_443_8, 0.012_456_8, 0.000_062_0),
    (0.97875, 0.047_352_9, 0.012_508_6, 0.000_040_5),
    (0.98, 0.042_262_0, 0.012_380_1, 0.000_199_2),
    (0.98125, 0.037_166_3, 0.012_157_5, 0.000_453_7),
    (0.9825, 0.031_912_0, 0.011_688_4, 0.001_007_5),
    (0.98375, 0.026_785_3, 0.010_833_3, 0.001_904_8),
    (0.985, 0.022_565_1, 0.009_999_9, 0.002_478_3),
    (0.98625, 0.019_542_1, 0.009_166_6, 0.002_652_7),
    (0.9875, 0.017_242_5, 0.008_333_3, 0.002_585_8),
    (0.98875, 0.014_925_4, 0.007_500_3, 0.002_525_0),
    (0.99, 0.012_281_3, 0.006_647_3, 0.002_592_7),
    (0.99125, 0.009_665_7, 0.005_839_7, 0.002_605_5),
    (0.9925, 0.007_637_9, 0.005_081_1, 0.002_373_3),
    (0.99375, 0.006_250_2, 0.004_321_1, 0.001_929_2),
    (0.995, 0.005_000_3, 0.003_574_2, 0.001_426_2),
    (0.99625, 0.003_750_3, 0.002_850_8, 0.000_899_5),
    (0.9975, 0.002_500_3, 0.002_132_4, 0.000_367_8),
    (0.99875, 0.001_250_2, 0.001_364_7, -0.000_114_5),
];

/// (α₂, α₃, β₃) at magnitude `m` from samples such as [`CURVATURE_SAMPLES`]: between samples, along the line between
/// them; past the last (towards 1, where the section closes up on the corner), the last's shrunk with 1 − m (not
/// sampled).
fn sampled_curvature(samples: &[(f64, f64, f64, f64)], m: f64) -> (f64, f64, f64) {
    let last = samples[samples.len() - 1];
    if m >= last.0 {
        let k = (1.0 - m) / (1.0 - last.0);
        return (last.1 * k, last.2 * k, last.3 * k);
    }
    let i = samples.iter().rposition(|s| s.0 <= m).unwrap_or(0);
    let (a, b) = (samples[i], samples[i + 1]);
    let f = (m - a.0) / (b.0 - a.0);
    let lerp = |x: f64, y: f64| x + (y - x) * f;
    (lerp(a.1, b.1), lerp(a.2, b.2), lerp(a.3, b.3))
}

/// A curvature section's poles, each as (s, t): the corner plus s times the way to its first end and t times the way
/// to its second ([`curvature_section`]), the corner's angle having cosine `cos`.
fn curvature_coefficients(m: f64, cos: f64) -> R<Vec<(f64, f64)>> {
    let (mut alpha, mut beta) = (2.0 * (1.0 - m) / 3.0, 1.0 / 3.0 - 7.0 * m / 15.0);
    let mut alpha2 = 1.0 - 0.6 * m;
    if beta < 0.0 {
        beta = -beta / 40.0;
        alpha = 1.0 - 17.0 * m / 15.0 - beta;
    }
    if m >= CURVATURE_SAMPLES[0].0 {
        if m >= 1.0 {
            return Err(error("@opFillet: a curvature fillet's magnitude must be under 1"));
        }
        let samples: &[(f64, f64, f64, f64)] = if cos.abs() < 1e-6 {
            &CURVATURE_SAMPLES
        } else if (cos * cos - 0.25).abs() < 1e-6 {
            &CURVATURE_SAMPLES_60
        } else {
            return Err(error(
                "@opFillet: curvature fillets of magnitude 0.975 and over at corners other than 60°, 90° and 120° aren't supported yet",
            ));
        };
        (alpha2, alpha, beta) = sampled_curvature(samples, m);
    } else if alpha < beta {
        alpha2 = 4.066_789_2 - 4.110_802_4 * m;
        beta = 0.017_222_28 - 0.017_589_06 * m;
        alpha = ((1.0 - m) / 2.0 - 0.125 * alpha2) / 0.375 - beta;
    }
    Ok(vec![
        (1.0, 0.0),
        (1.0 - 0.3 * m, 0.0),
        (alpha2, 0.0),
        (alpha, beta),
        (beta, alpha),
        (0.0, alpha2),
        (0.0, 1.0 - 0.3 * m),
        (0.0, 1.0),
    ])
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

/// For an edge of a sketch's region sheet, the edge of the sketch curve's own wire body on it: those follow the
/// order the sketch made its curves in, as the regions' edges don't.
fn sketch_curve_tid(model: &Model, tid: Tid) -> Option<Tid> {
    let r = model.find_tid(tid)?;
    let body = &model.bodies[r.body];
    if r.kind != EntityType::Edge || body.body_type != crate::model::BodyType::Sheet || body.sketch_plane.is_none() {
        return None;
    }
    let edge = model.shape(r);
    let middle = occt::vertex(edge.edge_tangent(0.5, false).ok()?.origin).ok()?;
    let created_by = &body.info.created_by;
    model
        .bodies
        .iter()
        .filter(|w| w.body_type == crate::model::BodyType::Wire && w.info.created_by == *created_by)
        .flat_map(|w| w.edges.iter())
        .find(|e| e.shape.distance(&middle).is_ok_and(|d| d.distance < 1e-9))
        .map(|e| e.tid)
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
        // An equal chamfer of circles between a plane and a sphere about their axis: a cone revolved round it, as
        // Onshape's (batch 125 c005; OCCT's is a B-spline).
        if let occt::Chamfer::Equal(width) = kind
            && let Some(op) = sphere_rim_blend(&body, &edges, RimBlend::Chamfer(width))?
        {
            results.push((b, op));
            continue;
        }
        // Along intersection curves (neither lines nor circles), Onshape's chamfer runs between the edges of the
        // fillet of its width (batch 129: a tee's joint, a cross hole's mouth); along lines and circles its feet are
        // its width from the edge, as OCCT's.
        if let occt::Chamfer::Equal(width) = kind
            && edges.iter().all(|e| !matches!(e.curve_type().ok(), Some(occt::CurveType::Line | occt::CurveType::Circle)))
            && let Some(op) = occt::chamfer_by_ball(&body, &edges, width)
        {
            results.push((b, op));
            continue;
        }
        let mut faces = Vec::new();
        for edge in &edges {
            // The first distance goes on the older of the edge's faces, by the oldest entity each came from (batch
            // 123: a hole's mouth on the block's top, a boss's foot on the block's top), a side an extrude swept from
            // a curve before a cap from the region it bounds (a cylinder's side before its caps; a block's side
            // swept from its sketch's first line before the caps, the caps before the others).
            let mut neighbours: Vec<(usize, Shape)> = body
                .ancestors(edge, ShapeKind::Face)
                .into_iter()
                .filter_map(|f| {
                    let index = model.bodies[b].faces.iter().position(|e| e.shape.is_same(&f))?;
                    Some((index, f))
                })
                .collect();
            neighbours.dedup_by(|a, b| a.1.is_same(&b.1));
            let all = &model.bodies[b].faces;
            let oldest = |e: &crate::model::Entity| {
                let tid = e.ancestry.all.iter().copied().chain(std::iter::once(e.tid)).min()?;
                Some(sketch_curve_tid(model, tid).unwrap_or(tid))
            };
            let key = |index: usize| {
                let entity = &all[index];
                if entity.info.cap.is_none() {
                    return (oldest(entity), false);
                }
                // A cap ranks just after its extrude's oldest side.
                let sides = all.iter().filter(|e| e.info.cap.is_none() && e.info.created_by == entity.info.created_by);
                (sides.filter_map(oldest).min().or(oldest(entity)), true)
            };
            neighbours.sort_by_key(|(index, _)| key(*index));
            let [(_, first), (_, second)] = &neighbours[..] else { return Err(regen_error("CHAMFER_FAILED")) };
            let mut face = first.clone();
            if opposite && matches!(kind, occt::Chamfer::DistanceAngle(..)) {
                face = second.clone();
            }
            faces.push(face);
        }
        // Uneven chamfers along intersection curves: the point off the first face by the second distance (or the
        // distance times the angle's tangent) and off the other by the first, its feet the chamfer's edges (batch
        // 130).
        let curved = edges.iter().all(|e| !matches!(e.curve_type().ok(), Some(occt::CurveType::Line | occt::CurveType::Circle)));
        let offsets = match kind {
            occt::Chamfer::TwoDistances(first, second) => Some((second, first)),
            occt::Chamfer::DistanceAngle(d, angle) => Some((d * angle.tan(), d)),
            occt::Chamfer::Equal(_) => None,
        };
        if curved && let Some((off1, off2)) = offsets {
            if let Some(op) = occt::chamfer_by_offsets(&body, &edges, &faces, off1, off2) {
                results.push((b, op));
                continue;
            }
        }
        let op = occt::chamfer(&body, &edges, &faces, kind).map_err(|_| regen_error("CHAMFER_FAILED"))?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    super::sense_from_older_faces(model, &id);
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
    // Each side's faces must belong to one body (batch 082 c004, c005: two upright sheets on a floor).
    for (key, error_name) in
        [("side1", "FACE_BLEND_LEFT_WALL_MULTIPLE_BODIES"), ("side2", "FACE_BLEND_RIGHT_WALL_MULTIPLE_BODIES")]
    {
        let mut bodies: Vec<usize> =
            query::evaluate(model, field(definition, key))?.into_iter().map(|r| r.body).collect();
        bodies.sort_unstable();
        bodies.dedup();
        if bodies.len() > 1 {
            return Err(regen_error(error_name));
        }
    }
    let plain = field(definition, "blendControlType").as_str().is_none_or(|c| c == "RADIUS")
        && !values::opt_bool(definition, "hasLimits")?.unwrap_or(false)
        && !values::opt_bool(definition, "tangentHoldLines")?.unwrap_or(false)
        && !values::opt_bool(definition, "conicHoldLines")?.unwrap_or(false)
        && !values::opt_bool(definition, "hasCliffEdges")?.unwrap_or(false)
        && !values::opt_bool(definition, "hasCaps")?.unwrap_or(false)
        && !values::opt_bool(definition, "asymmetric")?.unwrap_or(false)
        && field(definition, "trim").as_str().is_none_or(|t| t == "WALLS")
        && !values::opt_bool(definition, "detach")?.unwrap_or(false);
    if !plain {
        return planar_face_blend(model, id, definition);
    }
    let section = match field(definition, "crossSectionShape").as_str() {
        Some("CONIC") => {
            Section::Conic(field(definition, "rho").as_number().ok_or_else(|| regen_error("FACE_BLEND_FAILED"))?)
        }
        Some("CURVATURE") => Section::Curvature(
            field(definition, "magnitude").as_number().ok_or_else(|| regen_error("FACE_BLEND_FAILED"))?,
        ),
        _ => Section::Circular,
    };
    let radius = values::length(field(definition, "radius"), "opFaceBlend radius")?;
    let side = |key: &str| -> R<Vec<EntityRef>> {
        Ok(query::evaluate(model, field(definition, key))?.into_iter().filter(|r| r.kind == EntityType::Face).collect())
    };
    let (side1, side2) = (side("side1")?, side("side2")?);
    if side1.is_empty() || side2.is_empty() {
        return Err(regen_error("FACE_BLEND_SELECT_FACES"));
    }
    let shapes = |refs: &[EntityRef]| -> Vec<Shape> { refs.iter().map(|r| model.shape(*r).clone()).collect() };
    // A help point where a side's faces lie in parallel planes apart, so that the blend has a solution at each:
    // Onshape fails rather than pick one (batch 083 c001: a floor and a Π-shaped sheet's two walls).
    if !field(definition, "helpPointPosition").is_undefined() {
        for faces in [shapes(&side1), shapes(&side2)] {
            let planes: Vec<occt::Frame> = faces.iter().filter_map(|f| f.face_plane().ok()).collect();
            let apart = planes.iter().enumerate().any(|(i, a)| {
                planes[i + 1..].iter().any(|b| {
                    crate::values::dot(a.normal, b.normal).abs() > 1.0 - 1e-9
                        && crate::values::dot(crate::values::sub(b.origin, a.origin), a.normal).abs() > 1e-7
                })
            });
            if apart {
                return Err(regen_error("FACE_BLEND_FAILED"));
            }
        }
        // Or where the sides meet along separate curves (a floor through a cylinder lying along it, batch 079 c005).
        if let Ok(curves) = occt::section_edges(&shapes(&side1), &shapes(&side2)) {
            if separate_chains(&curves) > 1 {
                return Err(regen_error("FACE_BLEND_FAILED"));
            }
        }
    }
    // Sides each in one plane (of one face or several, batch 083 c003–c006: an upright split along its length):
    // each side's sense must point to where the other side is, as between single faces.
    let one_plane = |faces: &[Shape]| -> Option<(occt::Frame, Shape)> {
        let first = faces.first()?.face_plane().ok()?;
        faces
            .iter()
            .all(|f| {
                f.face_plane().is_ok_and(|p| {
                    crate::values::dot(p.normal, first.normal).abs() > 1.0 - 1e-9
                        && crate::values::dot(crate::values::sub(p.origin, first.origin), first.normal).abs() < 1e-9
                })
            })
            .then(|| (first, faces[0].clone()))
    };
    if let (Some((plane1, face1)), Some((plane2, face2))) = (one_plane(&shapes(&side1)), one_plane(&shapes(&side2))) {
        side_senses(definition, (&plane1, &face1), (&plane2, &face2))?;
    }
    let reverse1 = !values::opt_bool(definition, "flipSide1Normal")?.unwrap_or(false);
    let reverse2 = !values::opt_bool(definition, "flipSide2Normal")?.unwrap_or(false);
    if let Some(sheet) = floor_wall_blend(&shapes(&side1), reverse1, &shapes(&side2), reverse2, radius, section)? {
        let first = model.bodies[side1[0].body].clone();
        let mut bodies: Vec<usize> = side1.iter().chain(&side2).map(|r| r.body).collect();
        bodies.sort_unstable();
        bodies.dedup();
        // The sides' bodies' other faces stay, joined to it (batch 083 c002: the roof between two walls).
        let sides: Vec<Shape> = shapes(&side1).into_iter().chain(shapes(&side2)).collect();
        let others: Vec<Shape> = bodies
            .iter()
            .flat_map(|&b| model.bodies[b].faces.iter().map(|f| f.shape.clone()))
            .filter(|f| !sides.iter().any(|s| s.is_same(f)))
            .collect();
        let sheet = if others.is_empty() {
            sheet
        } else {
            let mut faces = sheet.sub_shapes(ShapeKind::Face);
            faces.extend(others);
            occt::sew_faces(&faces, 1e-6).map_err(|_| regen_error("FACE_BLEND_FAILED"))?
        };
        let info = crate::model::Info::new(id);
        model.remove_bodies(&bodies);
        let new = crate::model::NewBody {
            tid: Some(first.tid),
            body_type: crate::model::BodyType::Sheet,
            ..crate::model::NewBody::replacing(&first, sheet)
        };
        model.add_body(new, &crate::model::Origins::default(), &info, &|_| None);
        return Ok(Value::UNDEFINED);
    }
    if !matches!(section, Section::Circular) {
        return planar_face_blend(model, id, definition);
    }
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

/// How many separate chains some edges make, joined where their ends meet.
fn separate_chains(edges: &[Shape]) -> usize {
    let ends: Vec<Vec<occt::Vec3>> = edges
        .iter()
        .map(|e| [0.0, 1.0].iter().filter_map(|&t| e.edge_tangent(t, false).ok().map(|a| a.origin)).collect())
        .collect();
    let mut group: Vec<usize> = (0..edges.len()).collect();
    fn root(group: &mut [usize], mut i: usize) -> usize {
        while group[i] != i {
            i = group[i];
        }
        i
    }
    for i in 0..edges.len() {
        for j in i + 1..edges.len() {
            let meet =
                ends[i].iter().any(|p| ends[j].iter().any(|q| crate::values::norm(crate::values::sub(*p, *q)) < 1e-7));
            if meet {
                let (a, b) = (root(&mut group, i), root(&mut group, j));
                group[a] = b;
            }
        }
    }
    (0..edges.len()).filter(|&i| root(&mut group, i) == i).count()
}

/// Each side's sense (its normal, flipped if asked) must point to the side of it the blend is on, where the other
/// side goes (batch 054: Onshape refuses sheets apart whose flips std works out wrongly).
fn side_senses(definition: &Value, side1: (&occt::Frame, &Shape), side2: (&occt::Frame, &Shape)) -> R<()> {
    use crate::values::{add, cross, dot, normalize, scale, sub};
    let failed = || regen_error("FACE_BLEND_FAILED");
    let ((plane1, face1), (plane2, face2)) = (side1, side2);
    let Some(t) = normalize(cross(plane1.normal, plane2.normal)) else { return Ok(()) };
    let (n1, n2) = (plane1.normal, plane2.normal);
    let (h1, h2) = (dot(n1, plane1.origin), dot(n2, plane2.origin));
    let k = dot(n1, n2);
    let det = 1.0 - k * k;
    let c = add(scale(n1, (h1 - k * h2) / det), scale(n2, (h2 - k * h1) / det));
    let into = |plane: &occt::Frame, face: &Shape| -> R<occt::Vec3> {
        let u = normalize(cross(plane.normal, t)).ok_or_else(failed)?;
        let inner = face.face_inner_point().map_err(|_| failed())?;
        Ok(if dot(sub(inner, c), u) >= 0.0 { u } else { scale(u, -1.0) })
    };
    let (u1, u2) = (into(plane1, face1)?, into(plane2, face2)?);
    let flipped = |key: &str| -> R<bool> { Ok(values::opt_bool(definition, key)?.unwrap_or(false)) };
    let sense = |n: occt::Vec3, flip: bool| if flip { scale(n, -1.0) } else { n };
    if dot(sense(n1, flipped("flipSide1Normal")?), u2) < 0.0 {
        return Err(regen_error("FACE_BLEND_WRONG_LEFT_WALL_SENSE"));
    }
    if dot(sense(n2, flipped("flipSide2Normal")?), u1) < 0.0 {
        return Err(regen_error("FACE_BLEND_WRONG_RIGHT_WALL_SENSE"));
    }
    Ok(())
}

/// A rolling ball blend between a planar floor (side 1) and walls standing square on it (side 2: planes square to
/// it and cylinders about axes square to it, batch 071 c004–c007): along the walls' base curves, offset the radius
/// towards the blend over the floor, the ball's centre runs at the radius above it, sweeping a quarter cylinder
/// along each line and a quarter torus round each arc. The floor keeps what lies beyond the offset curves (the
/// pieces not touching the walls), the walls what lies more than the radius above the floor, all sewn into one
/// sheet. Each side's normal, reversed if asked, points away from the blend. `None` if the sides aren't so. A conic
/// or curvature section takes the quarter circle's place, its surface a B-spline as Onshape's is (batch 084 c007).
fn floor_wall_blend(
    side1: &[Shape],
    reverse1: bool,
    side2: &[Shape],
    reverse2: bool,
    r: f64,
    section: Section,
) -> R<Option<Shape>> {
    use crate::values::{add, cross, dot, norm, normalize, scale, sub};
    let failed = || regen_error("FACE_BLEND_FAILED");
    let [floor] = side1 else { return Ok(None) };
    let Ok(plane) = floor.face_plane() else { return Ok(None) };
    // A single planar wall is blended as two planes are, with their trims (batch 055 c004).
    if let [wall] = side2
        && wall.face_plane().is_ok()
    {
        return Ok(None);
    }
    let n_floor = if reverse1 { scale(plane.normal, -1.0) } else { plane.normal };
    // Towards the blend.
    let up = scale(n_floor, -1.0);
    let height = |p: occt::Vec3| dot(sub(p, plane.origin), up);
    // The section at a point of a wall's base, the wall towards the blend along `w`: a quarter circle from the floor
    // round to the wall, or the section's conic or curvature curve with the base its middle control point.
    let profile = |at: occt::Vec3, w: occt::Vec3| -> R<Shape> {
        let (on_floor, on_wall) = (add(at, scale(w, r)), add(at, scale(up, r)));
        match section {
            Section::Circular => {
                let centre = add(at, add(scale(w, r), scale(up, r)));
                let frame =
                    occt::Frame { origin: centre, normal: cross(scale(up, -1.0), scale(w, -1.0)), x: scale(up, -1.0) };
                occt::arc_edge(frame, r, 0.0, std::f64::consts::FRAC_PI_2).map_err(|_| failed())
            }
            Section::Conic(rho) => conic_edge(on_floor, at, on_wall, rho / (1.0 - rho)),
            Section::Curvature(m) => curvature_section(on_floor, at, on_wall, m),
        }
    };
    let finish = |piece: Shape| -> R<Shape> {
        if matches!(section, Section::Circular) {
            Ok(piece)
        } else {
            occt::nurbs_converted(&piece).map_err(crate::occt_error)
        }
    };
    let mut pieces = Vec::new();
    let mut offsets = Vec::new();
    let mut bases = Vec::new();
    for wall in side2 {
        let Ok(s) = wall.surface_definition() else { return Ok(None) };
        // The wall's normal towards the blend at a point.
        let towards = |p: occt::Vec3| -> Option<occt::Vec3> {
            let (fu, fv) = wall.face_fraction(p).ok()?;
            let n = wall.face_tangent_plane(fu, fv).ok()?.normal;
            Some(if reverse2 { n } else { scale(n, -1.0) })
        };
        // Its edge on the floor's plane.
        let base = wall.sub_shapes(ShapeKind::Edge).into_iter().find(|e| {
            [0.0, 0.5, 1.0].iter().all(|t| e.edge_tangent(*t, false).is_ok_and(|a| height(a.origin).abs() < 1e-9))
        });
        // (On the floor itself: sheets apart are blended otherwise, batch 054 c009.)
        let Some(base) = base.filter(|b| floor.distance(b).is_ok_and(|d| d.distance < 1e-9)) else { return Ok(None) };
        let a = base.edge_tangent(0.0, false).map_err(crate::occt_error)?;
        let b = base.edge_tangent(1.0, false).map_err(crate::occt_error)?;
        let Some(w) = towards(add(a.origin, scale(up, r))) else { return Ok(None) };
        match s.kind {
            0 if dot(s.frame.normal, up).abs() < 1e-9 && base.curve_type().ok() == Some(occt::CurveType::Line) => {
                let w = scale(s.frame.normal, if dot(s.frame.normal, w) >= 0.0 { 1.0 } else { -1.0 });
                // A quarter circle about the centre line, from the floor (−up) round to the wall (−w).
                let arc = profile(a.origin, w)?;
                pieces.push(finish(occt::prism(&arc, sub(b.origin, a.origin)).map_err(|_| failed())?.shape())?);
                offsets.push(occt::line_edge(add(a.origin, scale(w, r)), add(b.origin, scale(w, r))).map_err(|_| failed())?);
            }
            1 if norm(cross(s.frame.normal, up)) < 1e-9 => {
                let axis_point = add(s.frame.origin, scale(up, -height(s.frame.origin)));
                let radial = normalize(sub(a.origin, axis_point)).ok_or_else(failed)?;
                let outward = dot(radial, w) > 0.0;
                let w = if outward { radial } else { scale(radial, -1.0) };
                let arc = profile(a.origin, w)?;
                // How far round the axis the base runs, from its start towards its end.
                let closed = norm(sub(a.origin, b.origin)) < 1e-9;
                let turn = if closed {
                    2.0 * std::f64::consts::PI
                } else {
                    let (u, v) = (sub(a.origin, axis_point), sub(b.origin, axis_point));
                    let mut t = dot(cross(u, v), up).atan2(dot(u, v));
                    // The way the edge runs at its start.
                    let sense = dot(cross(u, a.direction), up);
                    if sense < 0.0 {
                        t = -t;
                    }
                    if t <= 0.0 {
                        t += 2.0 * std::f64::consts::PI;
                    }
                    if sense < 0.0 { -t } else { t }
                };
                let axis = occt::Axis { origin: axis_point, direction: up };
                pieces.push(finish(occt::revolve(&arc, axis, turn).map_err(|_| failed())?.shape())?);
                let start = add(a.origin, scale(w, r));
                let rho = norm(sub(start, axis_point));
                let x = normalize(sub(start, axis_point)).ok_or_else(failed)?;
                let (frame, lo, hi) = if turn > 0.0 {
                    (occt::Frame { origin: axis_point, normal: up, x }, 0.0, turn)
                } else {
                    (occt::Frame { origin: axis_point, normal: scale(up, -1.0), x }, 0.0, -turn)
                };
                offsets.push(occt::arc_edge(frame, rho, lo, hi).map_err(|_| failed())?);
            }
            _ => return Ok(None),
        }
        bases.push(base);
    }
    // The floor beyond the offset curves.
    let split = occt::split(std::slice::from_ref(floor), &offsets).map_err(|_| failed())?.shape();
    let kept_floor: Vec<Shape> = split
        .sub_shapes(ShapeKind::Face)
        .into_iter()
        .filter(|piece| bases.iter().all(|b| piece.distance(b).is_ok_and(|d| d.distance > 1e-7)))
        .collect();
    // The walls above the radius.
    let mut kept_walls = Vec::new();
    for wall in side2 {
        let bounds = wall.bounding_box(false).map_err(crate::occt_error)?;
        let size = norm(sub(bounds.max, bounds.min)) * 2.0 + 1e-3;
        let level = occt::Frame { origin: add(plane.origin, scale(up, r)), normal: up, x: plane.x };
        let cutter = occt::plane_face(level, -size, size, -size, size).map_err(crate::occt_error)?;
        let split = occt::split(std::slice::from_ref(wall), &[cutter]).map_err(|_| failed())?.shape();
        kept_walls.extend(
            split
                .sub_shapes(ShapeKind::Face)
                .into_iter()
                .filter(|p| p.face_inner_point().is_ok_and(|q| height(q) > r)),
        );
    }
    let mut all = kept_floor;
    all.extend(pieces);
    all.extend(kept_walls);
    let sheet = occt::sew_faces(&all, 1e-7).map_err(|_| failed())?;
    Ok(Some(sheet))
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
    // Each side one face, or coplanar faces of one body merged into one (batch 084 c001–c004: an upright split
    // along its length).
    let merged = |refs: &[EntityRef]| -> R<Option<Shape>> {
        match refs {
            [one] => Ok(Some(model.shape(*one).clone())),
            _ => {
                let shapes: Vec<Shape> = refs.iter().map(|r| model.shape(*r).clone()).collect();
                let joined = occt::sew_faces(&shapes, 1e-7).map_err(|_| failed())?;
                let faces = occt::merge_faces(&joined.sub_shapes(ShapeKind::Face)).map_err(|_| failed())?.shape();
                Ok(match &faces.sub_shapes(ShapeKind::Face)[..] {
                    [one] if one.face_plane().is_ok() => Some(one.clone()),
                    _ => None,
                })
            }
        }
    };
    let (Some(face1), Some(face2)) = (merged(&side1)?, merged(&side2)?) else {
        return Err(error("@opFaceBlend: blends other than between two single faces aren't supported yet"));
    };
    let (f1, f2) = (&side1[0], &side2[0]);
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
    // A straight edge of either side along the line: which side it lies on and how far from the line.
    // Only an edge on a side's boundary holds the blend; one between two of its faces is passed over (batch 084
    // c001–c004: a split 4 mm up an upright, the plain blend consuming it).
    let along = |key: &str| -> R<Option<(usize, f64)>> {
        for r in query::evaluate(model, field(definition, key))? {
            let body = &model.bodies[r.body];
            let edge_shape = model.shape(r);
            if body
                .faces
                .iter()
                .filter(|f| f.shape.sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(edge_shape)))
                .count()
                > 1
            {
                continue;
            }
            let Some((a, b)) =
                model.shape(r).edge_tangent(0.0, false).ok().zip(model.shape(r).edge_tangent(1.0, false).ok())
            else {
                continue;
            };
            let (a, b) = (a.origin, b.origin);
            if values::norm(cross(sub(b, a), t)) > 1e-9 * values::norm(sub(b, a)) {
                return Err(error(
                    "@opFaceBlend: hold lines and cliff edges other than straight along the blend aren't supported yet",
                ));
            }
            for (k, (plane, u)) in [(&plane1, u1), (&plane2, u2)].into_iter().enumerate() {
                if dot(sub(a, plane.origin), plane.normal).abs() < 1e-7 {
                    return Ok(Some((k, dot(sub(a, c), u))));
                }
            }
        }
        Ok(None)
    };
    // A tangent hold line sets the blend back to it on its side, the other side's setback in proportion (batch 079
    // c004: a hold line 4 mm up a side, a 4 mm blend; Onshape's surface a B-spline).
    let held =
        if values::opt_bool(definition, "tangentHoldLines")?.unwrap_or(false) { along("tangentEdges")? } else { None };
    // An inverse tangent hold line on a side's boundary leaves the blend nothing beyond it to run tangent to (batch
    // 080 c002); between two of its faces it is passed over as the others are (084 c002).
    if values::opt_bool(definition, "tangentHoldLines")?.unwrap_or(false) && along("inverseTangentEdges")?.is_some() {
        return Err(regen_error("FACE_BLEND_DIMENSION_TOO_LARGE"));
    }
    let (s1, s2) = match held {
        Some((0, h)) if h > 1e-9 => (h, s2 * h / s1),
        Some((_, h)) if h > 1e-9 => (s1 * h / s2, h),
        _ => (s1, s2),
    };
    // A conic hold line sets the blend back to it on its side alone, the other side's setback kept: the section the
    // circle's conic between unequal setbacks (batch 080 c001: an elliptical quarter 4 by 5).
    let conic_held =
        if values::opt_bool(definition, "conicHoldLines")?.unwrap_or(false) { along("conicEdges")? } else { None };
    let (s1, s2) = match conic_held {
        Some((0, h)) if h > 1e-9 => (h, s2),
        Some((_, h)) if h > 1e-9 => (s1, h),
        _ => (s1, s2),
    };
    let held = held.or(conic_held);
    // A cliff edge nearer the line than the blend would reach: the ball rests on it and on the other side (batch 079
    // c003: on the top edge of a side 3 mm high, radius 5, set back sqrt(21) on the other side).
    let cliff =
        if values::opt_bool(definition, "hasCliffEdges")?.unwrap_or(false) { along("cliffEdges")? } else { None };
    let cliff = match cliff {
        Some((k, h)) if h > 1e-9 && h < if k == 0 { s1 } else { s2 } => Some((k, h)),
        _ => None,
    };
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
    let mut short = trim == "SHORT";
    let (mut lo, mut hi) = if short { (lo1.max(lo2), hi1.min(hi2)) } else { (lo1.min(lo2), hi1.max(hi2)) };
    // Limit planes (batch 071 c001, c002): each keeps the side its normal (flipped if asked) points to, the blend
    // running only where they all do; the sides are notched only there, as with a short trim, and a planar face
    // closes each end a plane limits, between the sides and the blend.
    let mut limited = (false, false);
    if values::opt_bool(definition, "hasLimits")?.unwrap_or(false) {
        for (key, flip) in [("limitPlane1", "limitPlane1Flip"), ("limitPlane2", "limitPlane2Flip")] {
            let Some(r) = query::evaluate(model, field(definition, key))?.into_iter().next() else { continue };
            let limit = model.shape(r).face_plane().map_err(|_| failed())?;
            let n = if values::opt_bool(definition, flip)?.unwrap_or(false) { scale(limit.normal, -1.0) } else { limit.normal };
            let rate = dot(t, n);
            if rate.abs() < 1e-12 {
                return Err(failed());
            }
            let x = dot(sub(limit.origin, c), n) / rate;
            if rate > 0.0 && x > lo {
                lo = x;
                limited.0 = true;
            } else if rate < 0.0 && x < hi {
                hi = x;
                limited.1 = true;
            }
        }
        short = true;
    }
    if hi - lo < 1e-9 {
        return Err(failed());
    }
    // A cap across the blend where either side runs on past it is inconsistent: in its middle (batch 071 c003), or
    // past one side's end (batch 079 c001, c002).
    if values::opt_bool(definition, "hasCaps")?.unwrap_or(false) {
        for cap in field(definition, "caps").as_array().map(|a| a.to_vec()).unwrap_or_default() {
            for r in query::evaluate(model, field(&cap, "entity"))? {
                let Ok(cap_plane) = model.shape(r).face_plane() else {
                    return Err(error("@opFaceBlend: caps other than planes aren't supported yet"));
                };
                let rate = dot(t, cap_plane.normal);
                if rate.abs() < 1e-12 {
                    continue;
                }
                let x = dot(sub(cap_plane.origin, c), cap_plane.normal) / rate;
                if x > lo1.min(lo2) + 1e-9 && x < hi1.max(hi2) - 1e-9 {
                    return Err(regen_error("FACE_BLEND_INCONSISTENT_DATA"));
                }
            }
        }
        return Err(error("@opFaceBlend: caps at a blend's ends aren't supported yet"));
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
    // With a cliff edge, a circle of the radius about a point that far from the other side and from the edge.
    let (s1, s2, p1, p2) = match cliff {
        Some((k, h)) => {
            let radius = values::length(field(definition, "radius"), "opFaceBlend radius")?;
            let (u_edge, u_rest) = if k == 0 { (u1, u2) } else { (u2, u1) };
            let (sin, cos) = (angle.sin(), angle.cos());
            let reach = (2.0 * radius * h * sin - h * h * sin * sin).max(0.0).sqrt();
            let d = h * cos + reach;
            let (edge_point, rest_point) = (add(corner, scale(u_edge, h)), add(corner, scale(u_rest, d)));
            if k == 0 { (h, d, edge_point, rest_point) } else { (d, h, rest_point, edge_point) }
        }
        None => (s1, s2, p1, p2),
    };
    let curve = match section {
        _ if cliff.is_some() => {
            let (k, _) = cliff.ok_or_else(failed)?;
            let radius = values::length(field(definition, "radius"), "opFaceBlend radius")?;
            let (rest_point, u_rest) = if k == 0 { (p2, u2) } else { (p1, u1) };
            // The centre: off the other side by the radius, toward the edge's side.
            let off = normalize(sub(sub(p1, corner), scale(u_rest, dot(sub(p1, corner), u_rest))))
                .or_else(|| normalize(sub(sub(p2, corner), scale(u_rest, dot(sub(p2, corner), u_rest)))))
                .ok_or_else(failed)?;
            let centre = add(rest_point, scale(off, radius));
            let mid = add(centre, scale(normalize(sub(scale(add(p1, p2), 0.5), centre)).ok_or_else(failed)?, radius));
            occt::arc_through(p2, mid, p1).map_err(crate::occt_error)?
        }
        Section::Circular if held.is_some() => conic_edge(p2, corner, p1, (angle / 2.0).sin())?,
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
    // A hold line's blend is a B-spline surface in Onshape: the section's conic, carried straight along the line.
    if held.is_some() && cliff.is_none() && matches!(section, Section::Circular) {
        let w = (angle / 2.0).sin();
        let shift = scale(t, hi - lo);
        let poles = [p2, corner, p1, add(p2, shift), add(corner, shift), add(p1, shift)];
        blend_faces = vec![
            occt::rational_bspline_surface_face(
                &poles,
                &[1.0, w, 1.0, 1.0, w, 1.0],
                (2, 3),
                &[0.0, 0.0, 1.0, 1.0],
                &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                (1, 2),
            )
            .map_err(|_| failed())?,
        ];
    }
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
        // The ends limit planes cut: the section's region between the sides, at that end.
        for (cut, x) in [(limited.0, lo), (limited.1, hi)] {
            if !cut {
                continue;
            }
            let shift = super::translation(scale(t, x - lo));
            let at = add(c, scale(t, x));
            let section = occt::transform(&curve, &shift).map_err(crate::occt_error)?.shape();
            let edges = [
                occt::line_edge(at, add(at, scale(u2, s2))).map_err(crate::occt_error)?,
                section,
                occt::line_edge(add(at, scale(u1, s1)), at).map_err(crate::occt_error)?,
            ];
            faces.push(occt::fill_edges(&edges).map_err(|_| failed())?);
        }
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

/// A variable conic, curvature or asymmetric fillet of a straight edge between two planes (batches 072 c009, 073
/// c006–c009): the radii run from one end's to the other's, smoothly (3s² − 2s³) or straight, and each section is
/// the conic (or curvature section, or for asymmetric circular radii the conic of weight sin(θ/2)) between the
/// points its radii set the faces back to, as [`section_fillet`]'s (from the leading face, `radius` at a convex
/// edge). The setbacks are linear in the radii, and the sections' poles in the setbacks, so the fillet is exactly
/// the B-spline surface (rational for conics) of degree 3 along the edge: rows the edge's Bézier points, each with
/// its section's poles. The wedge it cuts off (its two strips on the faces and its two end caps) is taken away.
fn variable_section_fillet(model: &mut Model, id: FeatureId, definition: &Value, section: Section) -> R<Value> {
    use crate::values::{add, cross, dot, norm, normalize, scale, sub};
    let failed = || regen_error("FILLET_FAILED");
    if field(definition, "pointOnEdgeSettings").as_array().is_some_and(|a| !a.is_empty()) {
        return Err(error("@opFillet: radii at points on edges aren't supported yet"));
    }
    match section {
        Section::Conic(v) | Section::Curvature(v) if !(v > 0.0 && v < 1.0) => return Err(failed()),
        _ => {}
    }
    let key = if matches!(section, Section::Circular) { "radius" } else { "nonCircularRadius" };
    let radius = values::length(field(definition, key), "opFillet radius")
        .or_else(|_| values::length(field(definition, "radius"), "opFillet radius"))?;
    let asymmetric = values::opt_bool(definition, "isAsymmetric")?.unwrap_or(false);
    let other = if asymmetric { values::length(field(definition, "otherRadius"), "opFillet otherRadius")? } else { radius };
    let flip = values::opt_bool(definition, "flipAsymmetric")?.unwrap_or(false);
    let smooth = values::opt_bool(definition, "smoothTransition")?.unwrap_or(false);
    // Each vertex's radii (its other radius, asymmetric), flipped as asked.
    let mut at_vertex: Vec<(occt::Vec3, f64, f64)> = Vec::new();
    for setting in field(definition, "vertexSettings").as_array().map(|a| a.to_vec()).unwrap_or_default() {
        let r = values::length(field(&setting, "vertexRadius"), "opFillet vertexRadius")?;
        let o = if asymmetric {
            values::length(field(&setting, "vertexOtherRadius"), "opFillet vertexOtherRadius").unwrap_or(r)
        } else {
            r
        };
        let (r, o) = if field(&setting, "vertexFlipAsymmetric").as_bool() == Some(true) { (o, r) } else { (r, o) };
        for v in query::evaluate(model, field(&setting, "vertex"))? {
            if let Ok(p) = model.shape(v).vertex_point() {
                at_vertex.push((p, r, o));
            }
        }
    }
    let radii_at = |p: occt::Vec3| {
        let (r, o) = at_vertex.iter().find(|(q, _, _)| norm(sub(*q, p)) < 1e-9).map(|(_, r, o)| (*r, *o)).unwrap_or((radius, other));
        if flip { (o, r) } else { (r, o) }
    };
    let law = |a: f64, b: f64| if smooth { [a, a, b, b] } else { [a, a + (b - a) / 3.0, a + 2.0 * (b - a) / 3.0, b] };
    let groups = blend_edges(model, definition)?;
    if groups.is_empty() {
        return Err(regen_error("FILLET_SELECT_EDGES"));
    }
    let mut results = Vec::new();
    for (b, edges) in groups {
        let body = model.bodies[b].shape.clone();
        // Edges apart from each other each get a fillet of their own (batch 078 c008).
        let ends: Vec<occt::Vec3> = edges
            .iter()
            .flat_map(|e| [0.0, 1.0].map(|t| e.edge_tangent(t, false).map(|l| l.origin)))
            .collect::<Result<_, _>>()
            .map_err(crate::occt_error)?;
        // Two meeting at a vertex end in a mitre, their wedges overlapping there (batch 085 c005); three need a blend.
        if ends.iter().any(|p| ends.iter().filter(|q| norm(sub(*p, **q)) < 1e-9).count() > 2) {
            return Err(error(
                "@opFillet: variable non-circular fillets of three edges meeting at a vertex aren't supported yet",
            ));
        }
        // What to take away from convex edges, and add at concave ones.
        let (mut cut, mut join) = (Vec::new(), Vec::new());
        for edge in &edges {
            let mut faces = body.ancestors(edge, ShapeKind::Face);
            if faces.len() != 2 {
                return Err(failed());
            }
            if leading_face(&body, edge).is_some_and(|l| l.is_same(&faces[1])) {
                faces.swap(0, 1);
            }
            if edge.curve_type().ok() == Some(occt::CurveType::Circle) {
                let start = edge.edge_tangent(0.0, false).map_err(crate::occt_error)?.origin;
                let end = edge.edge_tangent(1.0, false).map_err(crate::occt_error)?.origin;
                let ((ra, oa), (rz, oz)) = (radii_at(start), radii_at(end));
                let (d1, d2) = if asymmetric { (law(oa, oz), law(ra, rz)) } else { (law(ra, rz), law(oa, oz)) };
                let (wedge, inside) = arc_variable_wedge(&body, edge, [&faces[0], &faces[1]], [d1, d2], section)?;
                if inside {
                    cut.push(wedge)
                } else {
                    join.push(wedge)
                }
                continue;
            }
            if edge.curve_type().ok() != Some(occt::CurveType::Line) {
                return Err(error("@opFillet: variable non-circular fillets of curved edges aren't supported yet"));
            }
            let a = edge.edge_tangent(0.0, false).map_err(crate::occt_error)?.origin;
            let z = edge.edge_tangent(1.0, false).map_err(crate::occt_error)?.origin;
            let t = normalize(sub(z, a)).ok_or_else(failed)?;
            let mid = scale(add(a, z), 0.5);
            let mut into = Vec::new();
            for face in &faces {
                let plane = face
                    .face_plane()
                    .map_err(|_| error("@opFillet: variable fillets between curved faces aren't supported yet"))?;
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
            let ((ra, oa), (rz, oz)) = (radii_at(a), radii_at(z));
            // At a concave edge the same wedge, between the faces and the fillet, is added rather than taken away
            // (batch 078 c006).
            let inside =
                body.contains_point(add(mid, scale(add(u1, u2), ra.min(oa).min(rz).min(oz) * 0.05))).unwrap_or(true);
            // Varying, the radii go the other way round from a constant asymmetric fillet's: `radius` at the leading
            // face (batch 073 c007: a block's front set back 2 to 4, its top 3 to 5).
            let (d1, d2) = if asymmetric { (law(oa, oz), law(ra, rz)) } else { (law(ra, rz), law(oa, oz)) };
            let (sin, cos) = (angle.sin(), angle.cos());
            // Setbacks on the leading face and the other, at each Bézier point.
            let s1: Vec<f64> = (0..4).map(|i| (d2[i] + d1[i] * cos) / sin).collect();
            let s2: Vec<f64> = (0..4).map(|i| (d1[i] + d2[i] * cos) / sin).collect();
            if s1.iter().chain(&s2).any(|x| !(*x > 1e-9)) {
                return Err(failed());
            }
            let base = |i: usize| add(a, scale(sub(z, a), i as f64 / 3.0));
            // The section's poles as (along u1, along u2) multiples of the setbacks, and weights.
            let (coefficients, weights, knots_v, degree_v): (Vec<(f64, f64)>, Vec<f64>, Vec<f64>, i32) = match section {
                Section::Curvature(m) => (
                    curvature_coefficients(m, cos)?.into_iter().map(|(s2c, s1c)| (s1c, s2c)).collect(),
                    Vec::new(),
                    CURVATURE_KNOTS.to_vec(),
                    5,
                ),
                Section::Conic(rho) => (
                    vec![(1.0, 0.0), (0.0, 0.0), (0.0, 1.0)],
                    vec![1.0, rho / (1.0 - rho), 1.0],
                    vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                    2,
                ),
                Section::Circular => (
                    vec![(1.0, 0.0), (0.0, 0.0), (0.0, 1.0)],
                    vec![1.0, (angle / 2.0).sin(), 1.0],
                    vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                    2,
                ),
            };
            let section_pole =
                |i: usize, (c1, c2): (f64, f64)| add(base(i), add(scale(u1, c1 * s1[i]), scale(u2, c2 * s2[i])));
            let nv = coefficients.len();
            let mut poles = Vec::new();
            let mut grid_weights = Vec::new();
            for i in 0..4 {
                for &c in &coefficients {
                    poles.push(section_pole(i, c));
                }
                grid_weights.extend(weights.iter().copied());
            }
            let knots_u = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
            let fillet = if weights.is_empty() {
                occt::bspline_surface_face(&poles, (4, nv), &knots_u, &knots_v, (3, degree_v))
            } else {
                occt::rational_bspline_surface_face(&poles, &grid_weights, (4, nv), &knots_u, &knots_v, (3, degree_v))
            }
            .map_err(|_| failed())?;
            let cubic = |points: Vec<occt::Vec3>| -> R<Shape> {
                occt::bspline_edge(&points, &[], &knots_u, 3, false).map_err(crate::occt_error)
            };
            let end_section = |i: usize| -> R<Shape> {
                let points: Vec<occt::Vec3> = coefficients.iter().map(|&c| section_pole(i, c)).collect();
                occt::bspline_edge(&points, &weights, &knots_v, degree_v, false).map_err(crate::occt_error)
            };
            let line = |p: occt::Vec3, q: occt::Vec3| occt::line_edge(p, q).map_err(crate::occt_error);
            let (f1a, f1z) = (section_pole(0, (1.0, 0.0)), section_pole(3, (1.0, 0.0)));
            let (f2a, f2z) = (section_pole(0, (0.0, 1.0)), section_pole(3, (0.0, 1.0)));
            let strip1 = occt::fill_edges(&[
                line(a, z)?,
                line(z, f1z)?,
                cubic((0..4).map(|i| section_pole(i, (1.0, 0.0))).collect())?,
                line(f1a, a)?,
            ])
            .map_err(|_| failed())?;
            let strip2 = occt::fill_edges(&[
                line(a, z)?,
                line(z, f2z)?,
                cubic((0..4).map(|i| section_pole(i, (0.0, 1.0))).collect())?,
                line(f2a, a)?,
            ])
            .map_err(|_| failed())?;
            let cap0 = occt::fill_edges(&[line(a, f1a)?, end_section(0)?, line(f2a, a)?]).map_err(|_| failed())?;
            let cap1 = occt::fill_edges(&[line(z, f1z)?, end_section(3)?, line(f2z, z)?]).map_err(|_| failed())?;
            let shell = occt::sew_faces(&[fillet, strip1, strip2, cap0, cap1], 1e-7).map_err(|_| failed())?;
            let mut wedge = occt::solid_from(&shell).map_err(|_| failed())?;
            if wedge.volume().map_err(crate::occt_error)? < 0.0 {
                wedge = wedge.reversed();
            }
            if inside { cut.push(wedge) } else { join.push(wedge) }
        }
        let (kind, tools) = match (cut.is_empty(), join.is_empty()) {
            (false, true) => (occt::BooleanKind::Subtraction, cut),
            (true, false) => (occt::BooleanKind::Union, join),
            _ => {
                return Err(error(
                    "@opFillet: variable non-circular fillets of convex and concave edges at once aren't supported yet",
                ));
            }
        };
        let op = occt::boolean(kind, std::slice::from_ref(&body), &tools, true).map_err(|_| failed())?;
        results.push((b, op));
    }
    replace_bodies(model, &id, results);
    Ok(Value::UNDEFINED)
}

/// The wedge a variable fillet takes away along an arc (or adds, at a concave one; batch 078 c007), between faces
/// that run straight away from it square to it at every point, such as a plane and a cylinder about the arc's axis:
/// at each point the section of a straight edge's fillet between the directions into the two faces, its setbacks
/// following the laws (Bézier values over the edge's parameter) `d`. The fillet surface carries the sections, its
/// rows cubic splines through them at 32 points along the arc (Onshape's is a fit too); strips of the faces and the
/// end sections close it. Whether the edge is convex comes with it.
#[allow(clippy::type_complexity)]
fn arc_variable_wedge(
    body: &Shape,
    edge: &Shape,
    faces: [&Shape; 2],
    d: [[f64; 4]; 2],
    section: Section,
) -> R<(Shape, bool)> {
    use crate::values::{add, cross, dot, normalize, scale, sub};
    let failed = || regen_error("FILLET_FAILED");
    let unsupported =
        || error("@opFillet: variable non-circular fillets of curved edges between these faces aren't supported yet");
    let bezier = |c: [f64; 4], t: f64| {
        let s = 1.0 - t;
        c[0] * s * s * s + 3.0 * c[1] * s * s * t + 3.0 * c[2] * s * t * t + c[3] * t * t * t
    };
    // The point at a fraction along the edge, and the directions into each face square to it there.
    let frame = |t: f64| -> R<(occt::Vec3, [occt::Vec3; 2])> {
        let at = edge.edge_tangent(t, false).map_err(crate::occt_error)?;
        let mut into = [at.direction; 2];
        for (k, face) in faces.iter().enumerate() {
            let (fu, fv) = face.face_fraction(at.origin).map_err(crate::occt_error)?;
            let n = face.face_tangent_plane(fu, fv).map_err(crate::occt_error)?.normal;
            let mut u = normalize(cross(n, at.direction)).ok_or_else(failed)?;
            let on = |d: occt::Vec3| {
                occt::vertex(add(at.origin, scale(d, 1e-6)))
                    .and_then(|v| v.distance(face))
                    .is_ok_and(|x| x.distance < 1e-7)
            };
            if !on(u) {
                u = scale(u, -1.0);
            }
            into[k] = u;
        }
        Ok((at.origin, into))
    };
    let (_, [u1, u2]) = frame(0.5)?;
    let angle = dot(u1, u2).clamp(-1.0, 1.0).acos();
    let (coefficients, weights, knots_v, degree_v): (Vec<(f64, f64)>, Vec<f64>, Vec<f64>, i32) = match section {
        Section::Curvature(m) => (
            curvature_coefficients(m, angle.cos())?.into_iter().map(|(s2c, s1c)| (s1c, s2c)).collect(),
            Vec::new(),
            CURVATURE_KNOTS.to_vec(),
            5,
        ),
        Section::Conic(rho) => (
            vec![(1.0, 0.0), (0.0, 0.0), (0.0, 1.0)],
            vec![1.0, rho / (1.0 - rho), 1.0],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            2,
        ),
        Section::Circular => (
            vec![(1.0, 0.0), (0.0, 0.0), (0.0, 1.0)],
            vec![1.0, (angle / 2.0).sin(), 1.0],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            2,
        ),
    };
    // A section pole at a fraction along the edge.
    let pole = |t: f64, (c1, c2): (f64, f64)| -> R<occt::Vec3> {
        let (p, [u1, u2]) = frame(t)?;
        let angle = dot(u1, u2).clamp(-1.0, 1.0).acos();
        let (sin, cos) = (angle.sin(), angle.cos());
        let (r1, r2) = (bezier(d[0], t), bezier(d[1], t));
        let (s1, s2) = ((r2 + r1 * cos) / sin, (r1 + r2 * cos) / sin);
        if !(s1 > 1e-9 && s2 > 1e-9) {
            return Err(failed());
        }
        Ok(add(p, add(scale(u1, c1 * s1), scale(u2, c2 * s2))))
    };
    // The setback points must lie on their faces: the faces run straight away from the edge.
    for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
        for (k, c) in [(0, (1.0, 0.0)), (1, (0.0, 1.0))] {
            let q = pole(t, c)?;
            if occt::vertex(q).and_then(|v| v.distance(faces[k])).map_err(crate::occt_error)?.distance > 1e-7 {
                return Err(unsupported());
            }
        }
    }
    const SAMPLES: usize = 32;
    let parameters: Vec<f64> = (0..=SAMPLES).map(|k| k as f64 / SAMPLES as f64).collect();
    let row = |c: (f64, f64)| -> R<occt::BSplineData> {
        let h = 1e-5;
        let points: Vec<occt::Vec3> = parameters.iter().map(|&t| pole(t, c)).collect::<R<_>>()?;
        let derivatives: Vec<occt::Vec3> = parameters
            .iter()
            .map(|&t| {
                let (a, b) = ((t - h).max(0.0), (t + h).min(1.0));
                Ok(scale(sub(pole(b, c)?, pole(a, c)?), 1.0 / (b - a)))
            })
            .collect::<R<_>>()?;
        // Each span the cubic Hermite segment between its ends, as Bézier spans joined (knots of multiplicity 3).
        let mut poles = vec![points[0]];
        let mut knots = vec![0.0; 4];
        for k in 0..SAMPLES {
            let span = parameters[k + 1] - parameters[k];
            poles.push(add(points[k], scale(derivatives[k], span / 3.0)));
            poles.push(sub(points[k + 1], scale(derivatives[k + 1], span / 3.0)));
            poles.push(points[k + 1]);
            knots.extend([parameters[k + 1]; 3]);
        }
        knots.push(1.0);
        Ok(occt::BSplineData { degree: 3, periodic: false, rational: false, poles, weights: Vec::new(), knots })
    };
    let rows: Vec<occt::BSplineData> = coefficients.iter().map(|&c| row(c)).collect::<R<_>>()?;
    let nu = rows[0].poles.len();
    if rows.iter().any(|r| r.poles.len() != nu || r.knots != rows[0].knots) {
        return Err(failed());
    }
    let nv = coefficients.len();
    let mut poles = Vec::with_capacity(nu * nv);
    let mut grid_weights = Vec::new();
    for i in 0..nu {
        for r in &rows {
            poles.push(r.poles[i]);
        }
        grid_weights.extend(weights.iter().copied());
    }
    let knots_u = rows[0].knots.clone();
    let fillet = if weights.is_empty() {
        occt::bspline_surface_face(&poles, (nu, nv), &knots_u, &knots_v, (3, degree_v))
    } else {
        occt::rational_bspline_surface_face(&poles, &grid_weights, (nu, nv), &knots_u, &knots_v, (3, degree_v))
    }
    .map_err(|_| failed())?;
    let curve_of =
        |r: &occt::BSplineData| occt::bspline_edge(&r.poles, &[], &r.knots, 3, false).map_err(crate::occt_error);
    let line = |p: occt::Vec3, q: occt::Vec3| occt::line_edge(p, q).map_err(crate::occt_error);
    // The setback curves on each face, whichever end of the section each is.
    let (first, last) = (&row((1.0, 0.0))?, &row((0.0, 1.0))?);
    let (a, z) = (pole(0.0, (0.0, 0.0))?, pole(1.0, (0.0, 0.0))?);
    let (f1a, f1z) = (first.poles[0], first.poles[nu - 1]);
    let (f2a, f2z) = (last.poles[0], last.poles[nu - 1]);
    // The strips of the faces between the edge and the setback curves: a planar one filled, any other ruled from
    // the edge to its setback curve (the faces run straight away from the edge).
    let edge_row = row((0.0, 0.0))?;
    // Round a whole circle the wedge closes on itself: a planar strip is the ring between the edge and its setback
    // curve, and there are no ends (batch 082: a cylinder's top edge).
    let closed = crate::values::norm(sub(a, z)) < 1e-9;
    let strip = |face: &Shape, far: &occt::BSplineData, fa: occt::Vec3, fz: occt::Vec3| -> R<Shape> {
        if closed && face.surface_type().ok() == Some(occt::SurfaceType::Plane) {
            let plane = face.face_plane().map_err(crate::occt_error)?;
            let regions = occt::sketch_regions(&[edge.clone(), curve_of(far)?], plane).map_err(|_| failed())?;
            // The ring: the region whose boundary has both curves.
            return regions.into_iter().find(|r| r.sub_shapes(ShapeKind::Wire).len() == 2).ok_or_else(failed);
        }
        if face.surface_type().ok() == Some(occt::SurfaceType::Plane) {
            return occt::fill_edges(&[edge.clone(), line(z, fz)?, curve_of(far)?, line(fa, a)?]).map_err(|_| failed());
        }
        let ruled: Vec<occt::Vec3> = (0..nu).flat_map(|i| [edge_row.poles[i], far.poles[i]]).collect();
        occt::bspline_surface_face(&ruled, (nu, 2), &knots_u, &[0.0, 0.0, 1.0, 1.0], (3, 1)).map_err(|_| failed())
    };
    let strip1 = strip(faces[0], first, f1a, f1z)?;
    let strip2 = strip(faces[1], last, f2a, f2z)?;
    let end_section = |t: f64| -> R<Shape> {
        let points: Vec<occt::Vec3> = coefficients.iter().map(|&c| pole(t, c)).collect::<R<_>>()?;
        occt::bspline_edge(&points, &weights, &knots_v, degree_v, false).map_err(crate::occt_error)
    };
    let mut faces = vec![fillet, strip1, strip2];
    if !closed {
        faces.push(occt::fill_edges(&[line(a, f1a)?, end_section(0.0)?, line(f2a, a)?]).map_err(|_| failed())?);
        faces.push(occt::fill_edges(&[line(z, f1z)?, end_section(1.0)?, line(f2z, z)?]).map_err(|_| failed())?);
    }
    let shell = occt::sew_faces(&faces, 1e-6).map_err(|_| failed())?;
    let mut wedge = occt::solid_from(&shell).map_err(|_| failed())?;
    if wedge.volume().map_err(crate::occt_error)? < 0.0 {
        wedge = wedge.reversed();
    }
    let (p, [u1, u2]) = frame(0.5)?;
    let inside = body
        .contains_point(add(p, scale(add(u1, u2), 0.01 * (bezier(d[0], 0.5).min(bezier(d[1], 0.5))))))
        .unwrap_or(true);
    Ok((wedge, inside))
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
