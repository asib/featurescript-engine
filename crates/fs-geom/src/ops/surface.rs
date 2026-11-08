//! Curve and surface operations: extracting wires and faces, intersections, fills, extending sheets, curves on
//! faces, boundary surfaces, offset curves, enclosures and projections.

use super::by_body;
use crate::model::{BodyType, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, field};
use crate::{occt_error, query, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Shape, ShapeKind};

fn shapes(model: &Model, query: &Value, kind: EntityType) -> R<Vec<Shape>> {
    Ok(query::evaluate(model, query)?.iter().filter(|r| r.kind == kind).map(|r| model.shape(*r).clone()).collect())
}

/// Faces a query finds, with the faces of any bodies it finds.
fn faces_of(model: &Model, query: &Value) -> R<Vec<Shape>> {
    let mut out = Vec::new();
    for r in query::evaluate(model, query)? {
        match r.kind {
            EntityType::Face => out.push(model.shape(r).clone()),
            EntityType::Body => out.extend(model.body(r).faces.iter().map(|f| f.shape.clone())),
            _ => {}
        }
    }
    Ok(out)
}

/// Whether two faces, edges or vertices occupy the same place: sample points of each lie on the other.
fn coincide(a: &Shape, b: &Shape) -> bool {
    let samples = |s: &Shape| -> Vec<occt::Vec3> {
        match s.kind() {
            Some(ShapeKind::Vertex) => s.vertex_point().into_iter().collect(),
            Some(ShapeKind::Edge) => {
                [0.0, 0.5, 1.0].iter().filter_map(|&t| s.edge_tangent(t, false).ok().map(|a| a.origin)).collect()
            }
            _ => s.face_inner_point().into_iter().collect(),
        }
    };
    let on = |points: Vec<occt::Vec3>, target: &Shape| {
        !points.is_empty()
            && points.iter().all(|p| occt::vertex(*p).and_then(|v| v.distance(target)).is_ok_and(|d| d.distance < 1e-9))
    };
    on(samples(a), b) && on(samples(b), a)
}

fn add(model: &mut Model, id: &FeatureId, shape: Shape, body_type: BodyType) {
    let info = Info::new(id.clone());
    model.add_body(NewBody::new(shape, body_type, info.clone()), &Origins::default(), &info, &|_| None);
}

/// Edges joined into wires, a wire body each.
fn add_wires(model: &mut Model, id: &FeatureId, edges: &[Shape]) -> R<()> {
    if edges.is_empty() {
        return Err(regen_error("EXTRACT_WIRES_FAILED"));
    }
    for wire in occt::connect_edges(edges).map_err(occt_error)? {
        add(model, id, wire, BodyType::Wire);
    }
    Ok(())
}

/// Curves a projection or intersection makes, each chain joined into one edge: a wire body each.
fn add_joined_curves(model: &mut Model, id: &FeatureId, edges: &[Shape]) -> R<()> {
    if edges.is_empty() {
        return Err(regen_error("EXTRACT_WIRES_FAILED"));
    }
    for wire in occt::connect_edges(edges).map_err(occt_error)? {
        let joined =
            if wire.sub_shapes(ShapeKind::Edge).len() > 1 { occt::join_edges(&wire).unwrap_or(wire) } else { wire };
        add(model, id, joined, BodyType::Wire);
    }
    Ok(())
}

/// `@opSplineThroughEdges`: one spline along a chain of edges: the edges joined into one B-spline.
pub fn spline_through_edges(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let edges = shapes(model, field(definition, "edges"), EntityType::Edge)?;
    add_joined_curves(model, &id, &edges)?;
    Ok(Value::UNDEFINED)
}

pub fn extract_wires(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let edges = shapes(model, field(definition, "edges"), EntityType::Edge)?;
    add_wires(model, &id, &edges)?;
    Ok(Value::UNDEFINED)
}

pub fn extract_surface(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    if values::opt_bool(definition, "tangentPropagation")?.unwrap_or(false) {
        return Err(error("@opExtractSurface: tangent propagation isn't supported yet"));
    }
    let offset = match field(definition, "offset") {
        v if v.is_undefined() => 0.0,
        v => values::length(v, "opExtractSurface offset")?,
    };
    let faces = faces_of(model, field(definition, "faces"))?;
    if faces.is_empty() {
        return Err(regen_error("EXTRACT_SURFACE_FAILED"));
    }
    // The copies' entities are tracked back to the faces, edges and vertices they lie on (std tracks a sketch's
    // lines into the surfaces extracted from its regions).
    let mut sources: Vec<crate::model::Entity> = Vec::new();
    if offset == 0.0 {
        for body in &model.bodies {
            for entity in body.all_entities() {
                if faces.iter().any(|f| {
                    f.is_same(&entity.shape)
                        || f.sub_shapes(entity.shape.kind().unwrap_or(ShapeKind::Face))
                            .iter()
                            .any(|x| x.is_same(&entity.shape))
                }) {
                    sources.push(entity.clone());
                }
            }
        }
    }
    for sheet in occt::extract_faces(&faces, offset).map_err(|_| regen_error("EXTRACT_SURFACE_FAILED"))? {
        let mut origins = Origins::default();
        for kind in [ShapeKind::Face, ShapeKind::Edge, ShapeKind::Vertex] {
            for copy in sheet.sub_shapes(kind) {
                for source in sources.iter().filter(|e| e.shape.kind() == Some(kind)) {
                    if coincide(&copy, &source.shape) {
                        origins.derive(&copy, source, true);
                    }
                }
            }
        }
        let info = Info::new(id.clone());
        model.add_body(NewBody::new(sheet, BodyType::Sheet, info.clone()), &origins, &info, &|_| None);
    }
    Ok(Value::UNDEFINED)
}

pub fn intersect_faces(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let tools = faces_of(model, field(definition, "tools"))?;
    let targets = faces_of(model, field(definition, "targets"))?;
    let edges = occt::section_edges(&tools, &targets).map_err(|_| regen_error("INTERSECTION_CURVE_FAILED"))?;
    if edges.is_empty() {
        return Err(regen_error("INTERSECTION_CURVE_NO_INTERSECTION"));
    }
    add_joined_curves(model, &id, &edges)?;
    Ok(Value::UNDEFINED)
}

pub fn fill_surface(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    for key in ["edgesG1", "edgesG2", "guideVertices"] {
        if !field(definition, key).is_undefined() && !query::evaluate(model, field(definition, key))?.is_empty() {
            return Err(error(format!("@opFillSurface: {key} isn't supported yet")));
        }
    }
    let mut edges = shapes(model, field(definition, "edgesG0"), EntityType::Edge)?;
    for r in query::evaluate(model, field(definition, "edgesG0"))? {
        if r.kind == EntityType::Body {
            edges.extend(model.body(r).edges.iter().map(|e| e.shape.clone()));
        }
    }
    let face = occt::fill_edges(&edges).map_err(|_| regen_error("FILL_SURFACE_FAILED"))?;
    add(model, &id, face, BodyType::Sheet);
    Ok(Value::UNDEFINED)
}

pub fn extend_sheet(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    if field(definition, "extendMethod").as_str() == Some("EXTEND_TO_SURFACE") {
        return super::sheet_metal::extend_to_plane(model, id, definition);
    }
    // Up to a target: std makes a plane body for a planar target face (batch 020).
    if field(definition, "endCondition").as_str() == Some("EXTEND_TO_TARGET") {
        let target = field(definition, "target").clone();
        return super::sheet_metal::extend_to_plane_with(model, id, definition, &target);
    }
    match field(definition, "endCondition").as_str() {
        None | Some("EXTEND_BLIND") => {}
        Some(other) => return Err(error(format!("@opExtendSheetBody: {other} isn't supported yet"))),
    }
    let distance = values::length(field(definition, "extendDistance"), "opExtendSheetBody extendDistance")?;
    let found = query::evaluate(model, field(definition, "entities"))?;
    // Edges by body; a whole body means all its edges.
    let mut groups: Vec<(usize, Vec<Shape>)> = by_body(model, field(definition, "entities"), EntityType::Edge)?;
    for r in found.iter().filter(|r| r.kind == EntityType::Body) {
        let edges: Vec<Shape> = model.bodies[r.body].edges.iter().map(|e| e.shape.clone()).collect();
        groups.push((r.body, edges));
    }
    let mut results = Vec::new();
    for (b, edges) in groups {
        let curving = field(definition, "extensionShape").as_str() == Some("SOFT");
        let shape = match extend_cylindrical(&model.bodies[b], &edges, distance)?
            .map(Ok)
            .or_else(|| extend_revolved(&model.bodies[b], &edges, distance).transpose())
            .transpose()?
        {
            Some(patch) => patch,
            None => match extend_extruded(&model.bodies[b], &edges, distance, curving)? {
                Some(sheet) => sheet,
                None => occt::extend_planar_sheet(&model.bodies[b].shape, &edges, distance)
                    .map_err(|_| regen_error("EXTEND_SHEET_BODY_FAILED"))?,
            },
        };
        results.push((b, shape));
    }
    let info = Info::new(id);
    for (b, shape) in results.into_iter().rev() {
        let old = model.bodies[b].clone();
        model.remove_bodies(&[b]);
        let mut origins = Origins::default();
        for entity in old.all_entities() {
            origins.keep(entity);
        }
        model.add_body(NewBody::replacing(&old, shape), &origins, &info, &|_| None);
    }
    Ok(Value::UNDEFINED)
}

/// A sheet of one face on a cone or sphere, bounded by two generators or meridians (a constant u) and two circles
/// round the axis (a constant v), extended as Onshape extends it (batch 039): each point of an edge carried the
/// distance along the surface's circle or line through it, square to the edge. Across a generator or meridian
/// every point goes the distance round its own circle, so the new edge is a curve (a turn of d / r at radius r):
/// on a quarter cone, Onshape's new edge reaches exactly 0.5 rad round at radius 10 for 5 mm. Past a circle the
/// edge moves the distance along the generators or meridians. (Onshape's new edges are its own approximations,
/// off this by up to 0.03 mm²; a sphere's extended round strays further, turning 1.106 rad at radius 5 for 5 mm.)
fn extend_revolved(body: &crate::model::Body, edges: &[Shape], distance: f64) -> R<Option<Shape>> {
    use crate::values::{dot, norm, scale, sub};
    let [face] = &body.faces[..] else { return Ok(None) };
    let face = &face.shape;
    let Ok(s) = face.surface_definition() else { return Ok(None) };
    if s.kind != 2 && s.kind != 3 {
        return Ok(None);
    }
    let ((u0, v0), (u1, v1)) = face.face_uv_bounds().map_err(occt_error)?;
    let mut extend = [false; 4];
    for e in edges {
        let mid = e.edge_tangent(0.5, false).map_err(occt_error)?.origin;
        let (fu, fv) = face.face_fraction(mid).map_err(occt_error)?;
        let near = |f: f64, at: f64| (f - at).abs() < 1e-6;
        match () {
            _ if near(fu, 0.0) => extend[0] = true,
            _ if near(fu, 1.0) => extend[1] = true,
            _ if near(fv, 0.0) => extend[2] = true,
            _ if near(fv, 1.0) => extend[3] = true,
            _ => return Ok(None),
        }
    }
    let failed = || regen_error("EXTEND_SHEET_BODY_FAILED");
    // The radius of the circle round the axis at v.
    let radius_at = |v: f64| -> R<f64> {
        let p = face.face_tangent_plane(0.5, (v - v0) / (v1 - v0)).map_err(|_| failed())?.origin;
        let d = sub(p, s.frame.origin);
        Ok(norm(sub(d, scale(s.frame.normal, dot(d, s.frame.normal)))))
    };
    // Along a cone's generators v is length; along a sphere's meridians, angle.
    let dv = if s.kind == 2 { distance } else { distance / s.radius1 };
    let (vb, vt) = (if extend[2] { v0 - dv } else { v0 }, if extend[3] { v1 + dv } else { v1 });
    let side = |v: f64, at: f64, out: bool, sign: f64| -> R<f64> {
        Ok(if out {
            let r = radius_at(v)?;
            if r < 1e-9 {
                return Err(failed());
            }
            at + sign * distance / r
        } else {
            at
        })
    };
    let samples = 32;
    let column = |at: f64, out: bool, sign: f64, up: bool| -> R<Vec<(f64, f64)>> {
        let n = if out { samples } else { 1 };
        (0..=n)
            .map(|i| {
                let f = i as f64 / n as f64;
                let v = if up { vb + f * (vt - vb) } else { vt - f * (vt - vb) };
                Ok((side(v, at, out, sign)?, v))
            })
            .collect()
    };
    let right = column(u1, extend[1], 1.0, true)?;
    let left = column(u0, extend[0], -1.0, false)?;
    let bottom = vec![(left[left.len() - 1].0, vb), (right[0].0, vb)];
    let top = vec![(right[right.len() - 1].0, vt), (left[0].0, vt)];
    // Past a sphere's top, Onshape keeps the old corner at the far end of the u range as a vertex, the meridian
    // there in two edges (batch 039 c004: 5 edges; the near side's, and a cone's either side, are one).
    let split = s.kind == 3 && extend[3] && !extend[1];
    let mut loops = vec![bottom];
    if split {
        loops.push(vec![(u1, vb), (u1, v1)]);
        loops.push(vec![(u1, v1), (u1, vt)]);
    } else {
        loops.push(right);
    }
    loops.push(top);
    loops.push(left);
    occt::face_from_uv(face, &loops).map(Some).map_err(|_| failed())
}

/// A sheet of one face extruded from a curve, bounded by two copies of the curve and two straight edges (batch 039):
/// at a straight edge the curve is extended as `trimCurve` extends it, straight on or keeping its curvature, and
/// extruded again; at a copy of the curve the extrusion is made longer.
fn extend_extruded(body: &crate::model::Body, edges: &[Shape], distance: f64, curving: bool) -> R<Option<Shape>> {
    use crate::values::{add, dot, norm, normalize, scale, sub};
    let [face] = &body.faces[..] else { return Ok(None) };
    if face.shape.surface_type().ok() != Some(occt::SurfaceType::Extrusion) {
        return Ok(None);
    }
    let own: Vec<Shape> = body.edges.iter().map(|e| e.shape.clone()).collect();
    let (lines, curves): (Vec<&Shape>, Vec<&Shape>) =
        own.iter().partition(|e| e.curve_type().ok() == Some(occt::CurveType::Line));
    let ([line, _], [bottom, top]) = (lines.as_slice(), curves.as_slice()) else { return Ok(None) };
    let ends = |e: &Shape| -> R<(occt::Vec3, occt::Vec3)> {
        Ok((
            e.edge_tangent(0.0, false).map_err(occt_error)?.origin,
            e.edge_tangent(1.0, false).map_err(occt_error)?.origin,
        ))
    };
    let (b0, b1) = ends(bottom)?;
    let (l0, l1) = ends(line)?;
    // The extrusion: from the bottom curve to the top one, along the straight edges.
    let mut vector = if norm(sub(l0, b0)) < 1e-9 || norm(sub(l0, b1)) < 1e-9 {
        sub(l1, l0)
    } else if norm(sub(l1, b0)) < 1e-9 || norm(sub(l1, b1)) < 1e-9 {
        sub(l0, l1)
    } else {
        return Ok(None);
    };
    let along = normalize(vector).ok_or_else(|| regen_error("EXTEND_SHEET_BODY_FAILED"))?;
    let mut profile = edge_as_cubic(bottom)?;
    let mut shift = 0.0;
    for e in edges {
        let (e0, e1) = ends(e)?;
        if e.is_same(top) {
            vector = add(vector, scale(along, distance));
        } else if e.is_same(bottom) {
            vector = add(vector, scale(along, distance));
            shift += distance;
        } else if e.curve_type().ok() == Some(occt::CurveType::Line) {
            // The straight edge at the curve's start or end.
            let start = *profile.poles.first().expect("poles");
            let at_start =
                [e0, e1].iter().any(|p| norm(sub(sub(*p, start), scale(along, dot(sub(*p, start), along)))) < 1e-7);
            profile = extended_cubic(profile, !at_start, curving, distance, true)?;
        } else {
            return Ok(None);
        }
    }
    let curve = profile.edge()?;
    let curve = if shift > 0.0 {
        occt::transform(&curve, &super::translation(scale(along, -shift))).map_err(occt_error)?.shape()
    } else {
        curve
    };
    let face = occt::prism(&curve, vector).map_err(|_| regen_error("EXTEND_SHEET_BODY_FAILED"))?.shape();
    // As a shell, as sheets are.
    let shells = occt::connected_shells(&face).map_err(occt_error)?;
    Ok(shells.into_iter().next())
}

/// A sheet of one cylindrical face bounded by its parameter lines, grown at the chosen edges: round the cylinder
/// by the distance as arc length, along it by the distance (batch 018: Onshape carries the cylinder on).
fn extend_cylindrical(body: &crate::model::Body, edges: &[Shape], distance: f64) -> R<Option<Shape>> {
    let [face] = &body.faces[..] else { return Ok(None) };
    let face = &face.shape;
    let Ok(s) = face.surface_definition() else { return Ok(None) };
    if s.kind != 1 {
        return Ok(None);
    }
    let ((u0, v0), (u1, v1)) = face.face_uv_bounds().map_err(occt_error)?;
    let (mut u, mut v) = ((u0, u1), (v0, v1));
    let du = distance / s.radius1;
    for e in edges {
        let mid = e.edge_tangent(0.5, false).map_err(occt_error)?.origin;
        let (fu, fv) = face.face_fraction(mid).map_err(occt_error)?;
        let near = |f: f64, at: f64| (f - at).abs() < 1e-6;
        match () {
            _ if near(fu, 0.0) => u.0 -= du,
            _ if near(fu, 1.0) => u.1 += du,
            _ if near(fv, 0.0) => v.0 -= distance,
            _ if near(fv, 1.0) => v.1 += distance,
            _ => return Ok(None),
        }
    }
    occt::surface_patch(face, u, v).map(Some).map_err(|_| regen_error("EXTEND_SHEET_BODY_FAILED"))
}

pub fn curves_on_face(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let groups = field(definition, "curveDefinition")
        .as_array()
        .ok_or_else(|| error("@opCreateCurvesOnFace: curveDefinition must be an array"))?
        .clone();
    for group in groups.iter() {
        let faces = shapes(model, field(group, "face"), EntityType::Face)?;
        let face = faces.first().ok_or_else(|| regen_error("CANNOT_RESOLVE_ENTITIES"))?.clone();
        let onshape_frame = query::evaluate(model, field(group, "face"))?
            .into_iter()
            .find(|r| r.kind == EntityType::Face)
            .and_then(|r| model.info(r).surface_frame);
        let creation = field(group, "creationType").as_str().unwrap_or_default().to_string();
        // DIR1 curves run along the face's v parameter at fixed u (on a cylinder, lines along it), DIR2 the other
        // way. (Batch 005, c007 and c008.)
        let along_u = !creation.starts_with("DIR1");
        // Equally spaced curves: across an open range at i / (n - 1), ends included (curves on the face's boundary
        // are then dropped); round a periodic one at (i + ½) / n. (Batch 005, c007 and c008.)
        let auto = creation.contains("AUTO_SPACED");
        // Round a cylinder or cone, Onshape's u runs from its coordinate system's x axis towards its y axis
        // (batches 005 and 018): the face fraction at each.
        let round_face = |u: f64| -> Option<f64> {
            let frame = onshape_frame?;
            let s = face.surface_definition().ok()?;
            if along_u || !matches!(s.kind, 1 | 2) {
                return None;
            }
            let y = values::cross(frame.normal, frame.x);
            let a = 2.0 * std::f64::consts::PI * u;
            let dir = values::add(values::scale(frame.x, a.cos()), values::scale(y, a.sin()));
            face.face_fraction(values::add(frame.origin, dir)).ok().map(|(fu, _)| fu)
        };
        let periodic = occt::face_periodic(&face, !along_u);
        let parameters: Vec<f64> = if auto {
            let n = field(group, "nCurves").as_number().unwrap_or(1.0).max(1.0) as usize;
            if periodic && round_face(0.0).is_some() {
                (0..n).filter_map(|i| round_face(i as f64 / n as f64)).collect()
            } else if periodic {
                (0..n).map(|i| (i as f64 + 0.5) / n as f64).collect()
            } else if n == 1 {
                vec![0.5]
            } else {
                (0..n).map(|i| i as f64 / (n - 1) as f64).collect()
            }
        } else {
            let given = values::numbers(field(group, "parameters"), "parameters")?;
            given.into_iter().map(|u| round_face(u).unwrap_or(u)).collect()
        };
        let boundary: Vec<Shape> = face.visible_sub_shapes(ShapeKind::Edge);
        let names = field(group, "names").as_array();
        for (i, t) in parameters.into_iter().enumerate() {
            let mut curve_id = id.to_vec();
            if let Some(name) = names.and_then(|n| n.get(i)).and_then(Value::as_str) {
                curve_id.push(name.into());
            }
            let edges = occt::iso_curves(&face, along_u, t).map_err(occt_error)?;
            let on_boundary = |e: &Shape| {
                e.edge_tangent(0.5, false).is_ok_and(|p| {
                    occt::vertex(p.origin)
                        .is_ok_and(|v| boundary.iter().any(|b| b.distance(&v).is_ok_and(|d| d.distance < 1e-8)))
                })
            };
            if auto && !edges.is_empty() && edges.iter().all(on_boundary) {
                continue;
            }
            add_wires(model, &curve_id.into(), &edges)?;
        }
    }
    Ok(Value::UNDEFINED)
}

pub fn boundary_surface(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let v = field(definition, "vProfileSubqueries");
    if v.as_array().is_some_and(|a| !a.is_empty()) {
        return Err(error("@opBoundarySurface: v profiles aren't supported yet"));
    }
    let u = field(definition, "uProfileSubqueries")
        .as_array()
        .ok_or_else(|| error("uProfileSubqueries must be an array"))?;
    let mut wires = Vec::new();
    for q in u.iter() {
        let edges = shapes(model, q, EntityType::Edge)?;
        let mut connected = occt::connect_edges(&edges).map_err(occt_error)?;
        if connected.len() != 1 {
            return Err(regen_error("BOUNDARY_SURFACE_FAILED"));
        }
        wires.push(connected.remove(0));
    }
    let op = occt::loft(&wires, false).map_err(|_| regen_error("BOUNDARY_SURFACE_FAILED"))?;
    for piece in op
        .shape()
        .sub_shapes(ShapeKind::Shell)
        .into_iter()
        .chain(op.shape().sub_shapes(ShapeKind::Face).into_iter().take(0))
    {
        add(model, &id, piece, BodyType::Sheet);
    }
    Ok(Value::UNDEFINED)
}

pub fn offset_curve_on_face(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let distance = values::length(field(definition, "distance"), "opOffsetCurveOnFace distance")?;
    let opposite = values::opt_bool(definition, "oppositeDirection")?.unwrap_or(false);
    let extend = values::opt_bool(definition, "extend")?.unwrap_or(false);
    let imprint = values::opt_bool(definition, "imprint")?.unwrap_or(false);
    let edges = shapes(model, field(definition, "edges"), EntityType::Edge)?;
    let target_refs: Vec<crate::model::EntityRef> = query::evaluate(model, field(definition, "targets"))?
        .into_iter()
        .filter(|r| r.kind == EntityType::Face)
        .collect();
    let mut made: Vec<(Shape, crate::model::EntityRef)> = Vec::new();
    for edge in &edges {
        let face = target_refs
            .iter()
            .copied()
            .find(|r| model.shape(*r).sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(edge)))
            .ok_or_else(|| regen_error("OFFSET_CURVE_ON_FACE_INVALID_TARGETS"))?;
        let signed = if opposite { -distance } else { distance };
        let offset = occt::offset_edge_on_planar_face(edge, model.shape(face), signed)
            .map_err(|_| regen_error("OFFSET_CURVE_ON_FACE_FAILED"))?;
        // Extended, a straight offset runs on to the face's boundary (batch 027).
        let offset = if extend { extended_across(&offset, model.shape(face))? } else { offset };
        made.push((offset, face));
    }
    // Splitting, the face is cut along the curves instead (batch 027).
    if imprint {
        let mut by_body: Vec<(usize, crate::model::EntityRef, Vec<Shape>)> = Vec::new();
        for (curve, face) in made {
            match by_body.iter_mut().find(|(_, f, _)| *f == face) {
                Some((_, _, curves)) => curves.push(curve),
                None => by_body.push((face.body, face, vec![curve])),
            }
        }
        let mut replacements = Vec::new();
        for (body, face, curves) in by_body {
            let op = occt::split_face_by_edges(&model.bodies[body].shape, model.shape(face), &curves)
                .map_err(|_| regen_error("OFFSET_CURVE_ON_FACE_FAILED"))?;
            replacements.push((body, op));
        }
        super::replace_bodies(model, &id, replacements);
        return Ok(Value::UNDEFINED);
    }
    add_wires(model, &id, &made.into_iter().map(|(curve, _)| curve).collect::<Vec<_>>())?;
    Ok(Value::UNDEFINED)
}

/// A straight curve on a planar face carried on both ways to the face's boundary: the part of its line inside the
/// face that holds it. Other curves are left as they are.
fn extended_across(curve: &Shape, face: &Shape) -> R<Shape> {
    if curve.curve_type().ok() != Some(occt::CurveType::Line) || face.face_plane().is_err() {
        return Ok(curve.clone());
    }
    let (a, b) = (
        curve.edge_tangent(0.0, false).map_err(occt_error)?.origin,
        curve.edge_tangent(1.0, false).map_err(occt_error)?.origin,
    );
    let d = values::normalize(values::sub(b, a)).ok_or_else(|| regen_error("OFFSET_CURVE_ON_FACE_FAILED"))?;
    let bb = face.bounding_box(false).map_err(occt_error)?;
    let reach = values::norm(values::sub(bb.max, bb.min)) + values::norm(values::sub(a, bb.min));
    let long = occt::line_edge(values::sub(a, values::scale(d, reach)), values::add(b, values::scale(d, reach)))
        .map_err(occt_error)?;
    let inside = occt::boolean(occt::BooleanKind::Intersection, &[long], std::slice::from_ref(face), false)
        .map_err(|_| regen_error("OFFSET_CURVE_ON_FACE_FAILED"))?;
    let mid = values::scale(values::add(a, b), 0.5);
    let pieces = inside.shape().sub_shapes(ShapeKind::Edge);
    let piece = pieces
        .iter()
        .find(|e| occt::vertex(mid).and_then(|v| v.distance(e)).is_ok_and(|d| d.distance < 1e-7))
        .or(pieces.first())
        .cloned()
        .unwrap_or_else(|| curve.clone());
    Ok(piece)
}

pub fn enclose(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let found = query::evaluate(model, field(definition, "entities"))?;
    let mut inputs: Vec<Shape> = Vec::new();
    for r in &found {
        match r.kind {
            EntityType::Body => inputs.push(model.body(*r).shape.clone()),
            EntityType::Face => inputs.push(model.shape(*r).clone()),
            _ => {}
        }
    }
    let solids = occt::enclose(&inputs).map_err(|_| regen_error("ENCLOSE_FAILED"))?;
    if solids.is_empty() {
        return Err(regen_error("ENCLOSE_NO_REGION"));
    }
    for solid in solids {
        add(model, &id, solid, BodyType::Solid);
    }
    Ok(Value::UNDEFINED)
}

pub fn drop_curve(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let edges = shapes(model, field(definition, "tools"), EntityType::Edge)?;
    let mut tool_edges = edges;
    for r in query::evaluate(model, field(definition, "tools"))? {
        if r.kind == EntityType::Body {
            tool_edges.extend(model.body(r).edges.iter().map(|e| e.shape.clone()));
        }
    }
    let targets = faces_of(model, field(definition, "targets"))?;
    let projection = field(definition, "projectionType");
    let direction = if projection.get("projectionType").is_some() || projection.as_str().is_some() {
        field(definition, "direction")
    } else {
        field(projection, "direction")
    };
    let direction = match direction {
        v if v.is_undefined() => occt::Vec3::new(0.0, 0.0, 0.0),
        v => values::vec3(v, "opDropCurve direction")?,
    };
    let projected =
        occt::project_edges(&tool_edges, &targets, direction).map_err(|_| regen_error("DROP_CURVE_FAILED"))?;
    add_joined_curves(model, &id, &projected)?;
    Ok(Value::UNDEFINED)
}

/// `@opRuledSurface`: a sheet swept from path edges by `width` along a direction: given (`ALIGNED_WITH_VECTOR`), or
/// from a reference face (`ANGLE_FROM_FACE`): the direction leaving the face across the edge, turned by `angle` about
/// the edge as the face runs along it (so −90° is along the face's normal). Like extruded edges, the sheet faces
/// along the edge's tangent × the direction.
pub fn ruled_surface(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let width = values::length(field(definition, "width"), "opRuledSurface width")?;
    let angle = match field(definition, "angle") {
        v if v.is_undefined() => 0.0,
        v => values::angle(v, "opRuledSurface angle")?,
    };
    let mut kind = field(definition, "ruledSurfaceType").as_str().unwrap_or("ANGLE_FROM_FACE").to_string();
    // No angle from the vector is along it (batch 018).
    if kind == "ANGLE_FROM_VECTOR" && angle.abs() < 1e-12 {
        kind = "ALIGNED_WITH_VECTOR".to_string();
    }
    let path = shapes(model, field(definition, "path"), EntityType::Edge)?;
    if path.is_empty() {
        return Err(regen_error("RULED_SURFACE_SELECT_EDGES"));
    }
    let references = match field(definition, "referenceFaces") {
        v if v.is_undefined() => Vec::new(),
        v => faces_of(model, v)?,
    };
    // The ruling's direction at a fraction `at` of the way along the edge (by length).
    let direction_at = |edge: &Shape, angle: f64, at: f64| -> R<occt::Vec3> {
        Ok(match kind.as_str() {
            "ALIGNED_WITH_VECTOR" => {
                values::normalize(values::vec3(field(definition, "ruledDirection"), "ruledDirection")?)
                    .ok_or_else(|| error("@opRuledSurface: zero direction"))?
            }
            "ANGLE_FROM_FACE" => {
                // The reference face next to this edge, the edge as that face uses it, and the face's normal there.
                let face = references
                    .iter()
                    .find(|f| f.sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(edge)))
                    .ok_or_else(|| regen_error("RULED_SURFACE_FAILED"))?;
                let used = face
                    .sub_shapes(ShapeKind::Edge)
                    .into_iter()
                    .find(|e| e.is_same(edge))
                    .ok_or_else(|| regen_error("RULED_SURFACE_FAILED"))?;
                let tangent = used.edge_tangent(at, true).map_err(occt_error)?;
                let (u, v) = face.face_fraction(tangent.origin).map_err(occt_error)?;
                let normal = face.face_tangent_plane(u, v).map_err(occt_error)?.normal;
                let away = values::scale(values::cross(normal, tangent.direction), -1.0);
                let axis = occt::Axis { origin: occt::Vec3::new(0.0, 0.0, 0.0), direction: tangent.direction };
                let r = super::rotation(axis, angle).m;
                occt::Vec3::new(
                    r[0] * away.x + r[1] * away.y + r[2] * away.z,
                    r[4] * away.x + r[5] * away.y + r[6] * away.z,
                    r[8] * away.x + r[9] * away.y + r[10] * away.z,
                )
            }
            // The vector turned about the edge by the angle, towards the outside of the reference face next to the
            // edge (batch 019).
            "ANGLE_FROM_VECTOR" => {
                let v = values::normalize(values::vec3(field(definition, "ruledDirection"), "ruledDirection")?)
                    .ok_or_else(|| error("@opRuledSurface: zero direction"))?;
                let tangent = edge.edge_tangent(at, true).map_err(occt_error)?;
                let outward = references
                    .iter()
                    .find(|f| f.sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(edge)))
                    .and_then(|face| {
                        let (u, w) = face.face_fraction(tangent.origin).ok()?;
                        face.face_tangent_plane(u, w).ok().map(|p| p.normal)
                    });
                let turned = |a: f64| {
                    let axis = occt::Axis { origin: occt::Vec3::new(0.0, 0.0, 0.0), direction: tangent.direction };
                    let r = super::rotation(axis, a).m;
                    occt::Vec3::new(
                        r[0] * v.x + r[1] * v.y + r[2] * v.z,
                        r[4] * v.x + r[5] * v.y + r[6] * v.z,
                        r[8] * v.x + r[9] * v.y + r[10] * v.z,
                    )
                };
                let (plus, minus) = (turned(angle), turned(-angle));
                match outward {
                    Some(n) if values::dot(minus, n) > values::dot(plus, n) => minus,
                    _ => plus,
                }
            }
            other => return Err(error(format!("@opRuledSurface: {other} isn't supported yet"))),
        })
    };
    let direction_of = |edge: &Shape, angle: f64| direction_at(edge, angle, 0.5);
    let overrides = vertex_overrides(model, definition)?;
    let cubic = values::opt_bool(definition, "useCubicInterpolation")?.unwrap_or(true);
    let mut sheets = Vec::new();
    // Planar faces from curved edges, by a point inside, with the frame Onshape gives their plane (x along the middle
    // ruling): evPlane centres its origin in the face's box in those axes (batch 040).
    let mut frames: Vec<(occt::Vec3, occt::Frame)> = Vec::new();
    for edge in &path {
        let direction = direction_of(edge, angle)?;
        let ruling = values::scale(direction, width);
        // The ruling at each end: an override there (batch 024), or the default.
        let ends = [0.0, 1.0].map(|t| edge.edge_tangent(t, false).map(|l| l.origin));
        let (a, b) = (ends[0].clone().map_err(occt_error)?, ends[1].clone().map_err(occt_error)?);
        let at = |p: occt::Vec3| -> R<Option<occt::Vec3>> {
            let Some(o) = overrides.iter().find(|o| values::norm(values::sub(o.vertex, p)) < 1e-7) else {
                return Ok(None);
            };
            Ok(Some(match (&o.up_to, o.direction, o.angle) {
                (Some(q), _, _) => values::sub(*q, p),
                (None, Some(d), _) => values::scale(d, o.width.unwrap_or(width)),
                (None, None, Some(a)) => values::scale(direction_of(edge, a)?, o.width.unwrap_or(width)),
                (None, None, None) => values::scale(direction, o.width.unwrap_or(width)),
            }))
        };
        let (start, end) = (at(a)?, at(b)?);
        // Along a curved edge, rulings normal or tangent to a face (or turned from a vector about the edge) turn
        // with it: normal to a round from its top arc, they fan out (batch 040).
        if edge.curve_type().ok() != Some(occt::CurveType::Line) && kind != "ALIGNED_WITH_VECTOR" {
            let width_at = |p: occt::Vec3| {
                overrides.iter().find(|o| values::norm(values::sub(o.vertex, p)) < 1e-7).and_then(|o| o.width)
            };
            let angle_at = |p: occt::Vec3| {
                overrides.iter().find(|o| values::norm(values::sub(o.vertex, p)) < 1e-7).and_then(|o| o.angle)
            };
            if overrides.iter().any(|o| {
                (values::norm(values::sub(o.vertex, a)) < 1e-7 || values::norm(values::sub(o.vertex, b)) < 1e-7)
                    && (o.up_to.is_some() || o.direction.is_some())
            }) {
                return Err(error("@opRuledSurface: up to or along overrides on curved edges aren't supported yet"));
            }
            let (w0, w1) = (width_at(a).unwrap_or(width), width_at(b).unwrap_or(width));
            let (a0, a1) = (angle_at(a).unwrap_or(angle), angle_at(b).unwrap_or(angle));
            let smooth = |t: f64| if cubic { t * t * (3.0 - 2.0 * t) } else { t };
            let far_at = |t: f64| -> R<occt::Vec3> {
                let s = smooth(t);
                let point = edge.edge_tangent(t, true).map_err(occt_error)?.origin;
                let d = direction_at(edge, a0 + (a1 - a0) * s, t)?;
                Ok(values::add(point, values::scale(d, w0 + (w1 - w0) * s)))
            };
            let n = 32;
            let parameters: Vec<f64> = (0..=n).map(|k| k as f64 / n as f64).collect();
            let points: Vec<occt::Vec3> = parameters.iter().map(|t| far_at(*t)).collect::<R<_>>()?;
            // The far curve's derivative at an end, by a second order one-sided difference.
            let slope = |t: f64, h: f64| -> R<occt::Vec3> {
                let (f0, f1, f2) = (far_at(t)?, far_at(t + h)?, far_at(t + 2.0 * h)?);
                Ok(values::scale(
                    values::add(values::sub(values::scale(f1, 4.0), values::scale(f0, 3.0)), values::scale(f2, -1.0)),
                    1.0 / (2.0 * h),
                ))
            };
            let far =
                crate::curve_extend::Cubic::interpolate(&parameters, &points, slope(0.0, 1e-3)?, slope(1.0, -1e-3)?)?;
            // Only rulings in one plane, making a planar face, are supported yet.
            let base = values::sub(points[n], points[0]);
            let normal = values::normalize(values::cross(values::sub(b, a), base))
                .or_else(|| values::normalize(values::cross(values::sub(points[0], a), base)))
                .ok_or_else(|| regen_error("RULED_SURFACE_FAILED"))?;
            let edge_points: Vec<occt::Vec3> = parameters
                .iter()
                .map(|t| edge.edge_tangent(*t, true).map(|l| l.origin))
                .collect::<Result<_, _>>()
                .map_err(occt_error)?;
            if points.iter().chain(&edge_points).any(|p| values::dot(values::sub(*p, a), normal).abs() > 1e-9) {
                return Err(error(
                    "@opRuledSurface: rulings that turn out of a plane along a curved edge aren't supported yet",
                ));
            }
            let far_edge = far.edge()?;
            let edges = [
                edge.clone(),
                occt::line_edge(b, points[n]).map_err(occt_error)?,
                far_edge,
                occt::line_edge(points[0], a).map_err(occt_error)?,
            ];
            let face = occt::fill_edges(&edges).map_err(|_| regen_error("RULED_SURFACE_FAILED"))?;
            // Facing along ruling × tangent at the middle, as a sheet swept along a straight edge does (the reverse
            // of OCCT's prism).
            let mid = direction_at(edge, angle, 0.5)?;
            let tangent = edge.edge_tangent(0.5, true).map_err(occt_error)?.direction;
            let facing = values::cross(mid, tangent);
            let face_normal = face.face_plane().map_err(occt_error)?.normal;
            let middle = edge.edge_tangent(0.5, true).map_err(occt_error)?.origin;
            frames.push((
                values::add(middle, values::scale(mid, width * 0.5)),
                occt::Frame { origin: middle, normal: face_normal, x: mid },
            ));
            sheets.push(if values::dot(face_normal, facing) < 0.0 { face.reversed() } else { face });
            continue;
        }
        if start.is_none() && end.is_none() {
            let op = occt::prism(edge, ruling).map_err(|_| regen_error("RULED_SURFACE_FAILED"))?;
            sheets.extend(op.shape().sub_shapes(ShapeKind::Face).into_iter().map(|f| f.reversed()));
            continue;
        }
        if edge.curve_type().ok() != Some(occt::CurveType::Line) {
            return Err(error("@opRuledSurface: vertex overrides are only supported on straight edges yet"));
        }
        // The rulings blend from one end to the other, smoothly (3t² − 2t³) or linearly: with the edge's points, a
        // cubic in t, so the face is a B-spline surface cubic along the edge and straight across.
        let (r0, r1) = (start.unwrap_or(ruling), end.unwrap_or(ruling));
        let blend = if cubic { [0.0, 0.0, 1.0, 1.0] } else { [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0] };
        let mut poles = Vec::new();
        for (i, w) in blend.iter().enumerate() {
            let p = values::add(a, values::scale(values::sub(b, a), i as f64 / 3.0));
            let r = values::add(r0, values::scale(values::sub(r1, r0), *w));
            poles.push(p);
            poles.push(values::add(p, r));
        }
        // Facing the way the ruled sheet from the default ruling would.
        let reference = occt::prism(edge, ruling).map_err(|_| regen_error("RULED_SURFACE_FAILED"))?.shape();
        let reference = reference
            .sub_shapes(ShapeKind::Face)
            .into_iter()
            .next()
            .ok_or_else(|| regen_error("RULED_SURFACE_FAILED"))?;
        let facing = reference.reversed().face_plane().map_err(occt_error)?.normal;
        let faces_along = |face: &Shape| -> R<bool> {
            let mid = values::scale(values::add(a, b), 0.5);
            let (u, v) = face.face_fraction(mid).map_err(occt_error)?;
            Ok(values::dot(face.face_tangent_plane(u, v).map_err(occt_error)?.normal, facing) > 0.0)
        };
        // Rulings in one plane make a planar face, as in Onshape (batch 024).
        let plane_normal = values::normalize(values::cross(r0, values::sub(b, a)))
            .or_else(|| values::normalize(values::cross(r1, values::sub(b, a))));
        if let Some(n) = plane_normal
            && poles.iter().all(|p| values::dot(values::sub(*p, a), n).abs() < 1e-9)
        {
            let far: Vec<occt::Vec3> = poles.iter().skip(1).step_by(2).rev().copied().collect();
            let edges = [
                edge.clone(),
                occt::line_edge(b, far[0]).map_err(occt_error)?,
                occt::bezier_edge(&far).map_err(occt_error)?,
                occt::line_edge(far[3], a).map_err(occt_error)?,
            ];
            let face = occt::fill_edges(&edges).map_err(|_| regen_error("RULED_SURFACE_FAILED"))?;
            let normal = face.face_plane().map_err(occt_error)?.normal;
            sheets.push(if values::dot(normal, facing) < 0.0 { face.reversed() } else { face });
            continue;
        }
        let face = occt::bspline_surface_face(
            &poles,
            (4, 2),
            &[0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            &[0.0, 0.0, 1.0, 1.0],
            (3, 1),
        )
        .map_err(|_| regen_error("RULED_SURFACE_FAILED"))?;
        sheets.push(if faces_along(&face)? { face } else { face.reversed() });
    }
    let joined = occt::extract_faces(&sheets, 0.0).map_err(occt_error)?;
    let info = Info::new(id.clone());
    let framed = |s: &Shape| -> Option<std::rc::Rc<Info>> {
        let (_, frame) = frames.iter().find(|(p, _)| {
            s.kind() == Some(ShapeKind::Face)
                && occt::vertex(*p).and_then(|v| v.distance(s)).is_ok_and(|d| d.distance < 1e-9)
        })?;
        Some(std::rc::Rc::new(Info { created_by: id.clone(), surface_frame: Some(*frame), ..Default::default() }))
    };
    for sheet in joined {
        model.add_body(NewBody::new(sheet, BodyType::Sheet, info.clone()), &Origins::default(), &info, &framed);
    }
    // std reads the ruled bases only to place manipulators.
    Ok(Value::array(Vec::new()))
}

/// `@opMoveCurveBoundary`: extends straight wire bodies by a distance at the end nearer the help point (or the
/// other end with `flipHeuristics`).
pub fn move_curve_boundary(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let trim = field(definition, "moveBoundaryType").as_str() == Some("TRIM");
    let up_to = !trim && field(definition, "endCondition").as_str() == Some("UP_TO_ENTITY");
    // What to trim or extend to: a plane (batch 018), or a straight curve, taken as the plane through it square to
    // the curve being changed (batch 021).
    let limit_curve: Option<Shape>;
    let mut limit_surface: Option<Shape> = None;
    let limit = if trim || up_to {
        let key = if trim { "trimTo" } else { "extendTo" };
        let found = query::evaluate(model, field(definition, key))?;
        let face = found.iter().find_map(|r| match r.kind {
            EntityType::Face => Some(model.shape(*r).clone()),
            EntityType::Body => model.bodies[r.body].faces.first().map(|f| f.shape.clone()),
            _ => None,
        });
        limit_curve = if face.is_none() {
            found.iter().find_map(|r| match r.kind {
                EntityType::Edge => Some(model.shape(*r).clone()),
                EntityType::Body => model.bodies[r.body].edges.first().map(|e| e.shape.clone()),
                _ => None,
            })
        } else {
            None
        };
        match face {
            // A curved face is met where the curve's line crosses it (batch 027).
            Some(face) => match face.face_plane() {
                Ok(plane) => Some(plane),
                Err(_) => {
                    limit_surface = Some(face);
                    None
                }
            },
            None if limit_curve.is_some() => None,
            None => return Err(regen_error("CANNOT_RESOLVE_ENTITIES")),
        }
    } else {
        limit_curve = None;
        None
    };
    let distance = match (limit, &limit_curve, &limit_surface) {
        (None, None, None) => values::length(field(definition, "extensionDistance"), "extensionDistance")?,
        _ => 0.0,
    };
    let flip = values::opt_bool(definition, "flipHeuristics")?.unwrap_or(false);
    let help = query::evaluate(model, field(definition, "helpPoint"))?;
    let help = help.first().map(|r| model.shape(*r).clone());
    let bodies = super::owner_bodies(model, field(definition, "wires"))?;
    let mut results = Vec::new();
    for &b in &bodies {
        let body = model.bodies[b].clone();
        let edges = body.shape.sub_shapes(ShapeKind::Edge);
        let single_line = matches!(edges.as_slice(), [e] if e.curve_type().ok() == Some(occt::CurveType::Line));
        if !single_line {
            let curving = field(definition, "extensionShape").as_str() == Some("CURVATURE");
            let moved = curved_boundary_moved(
                &edges,
                trim,
                distance,
                limit,
                &limit_surface,
                limit_curve.as_ref(),
                help.as_ref(),
                flip,
                curving,
            )?;
            results.push((b, moved));
            continue;
        }
        let edge = &edges[0];
        let start = edge.edge_tangent(0.0, false).map_err(occt_error)?;
        let end = edge.edge_tangent(1.0, false).map_err(occt_error)?;
        let near_end = match &help {
            Some(h) => {
                let d = |p: occt::Vec3| {
                    occt::vertex(p).ok().and_then(|v| v.distance(h).ok()).map_or(f64::INFINITY, |d| d.distance)
                };
                d(end.origin) <= d(start.origin)
            }
            None => true,
        } != flip;
        // A straight limit curve: the plane through it, square to this curve within the plane they share.
        let limit = match (&limit, &limit_curve) {
            (Some(p), _) => Some(*p),
            (None, Some(curve)) => {
                let (c0, c1) = (
                    curve.edge_tangent(0.0, false).map_err(occt_error)?.origin,
                    curve.edge_tangent(1.0, false).map_err(occt_error)?.origin,
                );
                let along = values::normalize(values::sub(c1, c0)).ok_or_else(|| regen_error("TRIM_CURVE_FAILED"))?;
                let d = values::normalize(values::sub(end.origin, start.origin))
                    .ok_or_else(|| regen_error("TRIM_CURVE_FAILED"))?;
                let shared = values::cross(along, d);
                let normal =
                    values::normalize(values::cross(shared, along)).ok_or_else(|| regen_error("TRIM_CURVE_FAILED"))?;
                Some(occt::Frame { origin: c0, normal, x: along })
            }
            _ => None,
        };
        // Where the line meets the limit: a plane, or a curved face, the crossing nearest the end that moves.
        let hit = if let Some(plane) = limit {
            let d = values::sub(end.origin, start.origin);
            let rate = values::dot(d, plane.normal);
            if rate.abs() < 1e-12 {
                return Err(regen_error("TRIM_CURVE_FAILED"));
            }
            let t = values::dot(values::sub(plane.origin, start.origin), plane.normal) / rate;
            Some(values::add(start.origin, values::scale(d, t)))
        } else if let Some(surface) = &limit_surface {
            let length = values::norm(values::sub(end.origin, start.origin));
            let d = values::normalize(values::sub(end.origin, start.origin))
                .ok_or_else(|| regen_error("TRIM_CURVE_FAILED"))?;
            let crossings = surface.line_crossings(start.origin, d).map_err(occt_error)?;
            let moving = if near_end { length } else { 0.0 };
            let t = crossings
                .iter()
                .copied()
                .filter(|t| {
                    if trim {
                        *t > 1e-9 && *t < length - 1e-9
                    } else if near_end {
                        *t > length
                    } else {
                        *t < 0.0
                    }
                })
                .min_by(|a, b| (a - moving).abs().total_cmp(&(b - moving).abs()))
                .ok_or_else(|| regen_error("TRIM_CURVE_FAILED"))?;
            Some(values::add(start.origin, values::scale(d, t)))
        } else {
            None
        };
        let (a, b2) = if let Some(hit) = hit {
            match (trim, near_end) {
                // Trimming keeps the side the help point is on; extending moves the end nearest it.
                (true, true) | (false, true) => {
                    (if trim { hit } else { start.origin }, if trim { end.origin } else { hit })
                }
                (true, false) | (false, false) => {
                    (if trim { start.origin } else { hit }, if trim { hit } else { end.origin })
                }
            }
        } else if near_end {
            (start.origin, values::add(end.origin, values::scale(end.direction, distance)))
        } else {
            (values::sub(start.origin, values::scale(start.direction, distance)), end.origin)
        };
        results.push((b, occt::line_edge(a, b2).map_err(occt_error)?));
    }
    let info = Info::new(id);
    for (b, edge) in results.into_iter().rev() {
        let old = model.bodies[b].clone();
        model.remove_bodies(&[b]);
        model.add_body(NewBody::replacing(&old, edge), &Origins::default(), &info, &|_| None);
    }
    Ok(Value::UNDEFINED)
}

/// A curve as Onshape extends or trims it (batches 036, 038): an arc or elliptical arc made a B-spline, or the
/// clamped cubic B-spline it is.
fn edge_as_cubic(edge: &Shape) -> R<crate::curve_extend::Cubic> {
    use crate::curve_extend::{Cubic, arc_frame};
    use crate::values::{add, cross, dot, norm, scale, sub};
    let start = edge.edge_tangent(0.0, false).map_err(occt_error)?;
    let end = edge.edge_tangent(1.0, false).map_err(occt_error)?;
    if edge.curve_type().ok() == Some(occt::CurveType::Circle) {
        let centre = edge.curve_definition().map_err(occt_error)?.frame.origin;
        let (x, y, radius, angle) = arc_frame(centre, start.origin, start.direction, end.origin)?;
        return Cubic::arc(centre, x, y, radius, angle);
    }
    // An elliptical arc keeps the ellipse's own angle, from its major axis (batch 038).
    if edge.curve_type().ok() == Some(occt::CurveType::Ellipse) {
        let conic = edge.curve_definition().map_err(occt_error)?;
        let (centre, x, a, b) = (conic.frame.origin, conic.frame.x, conic.radius1, conic.radius2);
        let mut y = cross(conic.frame.normal, x);
        let angle_of = |p: occt::Vec3, y: occt::Vec3| {
            let d = sub(p, centre);
            (dot(d, y) / b).atan2(dot(d, x) / a)
        };
        let t0 = angle_of(start.origin, y);
        let along = add(scale(x, -a * t0.sin()), scale(y, b * t0.cos()));
        if dot(along, start.direction) < 0.0 {
            y = scale(y, -1.0);
        }
        let t0 = angle_of(start.origin, y);
        let mut t1 = angle_of(end.origin, y);
        while t1 <= t0 + 1e-12 {
            t1 += 2.0 * std::f64::consts::PI;
        }
        return Cubic::ellipse(centre, x, y, a, b, t0, t1);
    }
    let data = edge.edge_bspline(1e-7, false, false).map_err(occt_error)?;
    let first = data.knots.first().copied().unwrap_or_default();
    let last = data.knots.last().copied().unwrap_or_default();
    let clamped = data.knots.len() == data.poles.len() + 4
        && data.knots[..4].iter().all(|k| *k == first)
        && data.knots[data.knots.len() - 4..].iter().all(|k| *k == last);
    if data.degree != 3 || data.rational || data.periodic || !clamped {
        return Err(error(
            "@opMoveCurveBoundary: of curves other than lines, only arcs and clamped cubic B-splines are supported yet",
        ));
    }
    let cubic = Cubic { knots: data.knots, poles: data.poles };
    if norm(sub(*cubic.poles.last().expect("poles"), end.origin)) > 1e-9
        || norm(sub(cubic.poles[0], start.origin)) > 1e-9
    {
        return Err(error("@opMoveCurveBoundary: B-splines running against their edge aren't supported yet"));
    }
    Ok(cubic)
}

/// A cubic extended at its end (or its start, as its reverse is at its end, its parameters kept) by `distance`:
/// straight on, or keeping its curvature with a blend first (batches 036, 038). The distance runs along the curve,
/// or, with `by_chord`, straight from the old end to the new one, as a sheet's curving extension measures it (batch
/// 039: an extruded spline's extension 1.24e-4 mm longer than 5 mm along it).
fn extended_cubic(
    cubic: crate::curve_extend::Cubic,
    at_end: bool,
    curving: bool,
    distance: f64,
    by_chord: bool,
) -> R<crate::curve_extend::Cubic> {
    use crate::values::{norm, sub};
    let cubic = if at_end { cubic } else { cubic.reversed() };
    let old_end = cubic.derivative(cubic.end(), 0);
    let (base, already) = match if curving { cubic.blended()? } else { None } {
        Some(blended) => {
            let length = blended.length(cubic.end(), blended.end());
            (blended, length)
        }
        None => (cubic, 0.0),
    };
    let mut straight = distance - already;
    if straight <= 1e-12 {
        return Err(error("extensions ending within their blend aren't supported yet"));
    }
    let mut extended = base.straight_on(straight)?;
    if by_chord {
        for _ in 0..20 {
            let chord = norm(sub(extended.derivative(extended.end(), 0), old_end));
            if (distance - chord).abs() < 1e-15 {
                break;
            }
            straight += distance - chord;
            extended = base.straight_on(straight)?;
        }
    }
    Ok(if at_end { extended } else { extended.reversed() })
}

/// A curve other than a single line trimmed or extended (batch 036): an arc trimmed by a plane stays an arc; a curve
/// extended at its end (an arc, a cubic B-spline, or a chain of edges ending in one) becomes a B-spline as Onshape
/// makes it (see `curve_extend`), a line at a chain's end staying a line.
#[allow(clippy::too_many_arguments)]
fn curved_boundary_moved(
    edges: &[Shape],
    trim: bool,
    distance: f64,
    limit: Option<occt::Frame>,
    limit_surface: &Option<Shape>,
    limit_curve: Option<&Shape>,
    help: Option<&Shape>,
    flip: bool,
    curving: bool,
) -> R<Shape> {
    use crate::curve_extend::arc_frame;
    use crate::values::{add, cross, dot, norm, normalize, scale, sub};
    if limit_curve.is_some_and(|c| c.curve_type().ok() != Some(occt::CurveType::Line)) {
        return Err(error(
            "@opMoveCurveBoundary: curves other than lines up to curves other than lines aren't supported yet",
        ));
    }
    // The chain's ends: points only one edge reaches.
    let points: Vec<(occt::Axis, occt::Axis)> = edges
        .iter()
        .map(|e| Ok((e.edge_tangent(0.0, false)?, e.edge_tangent(1.0, false)?)))
        .collect::<Result<_, occt::Error>>()
        .map_err(occt_error)?;
    let mut ends: Vec<(usize, bool, occt::Vec3)> = Vec::new();
    for (i, (a, b)) in points.iter().enumerate() {
        for (at_end, p) in [(false, a.origin), (true, b.origin)] {
            let shared = points
                .iter()
                .enumerate()
                .any(|(j, (c, d))| j != i && (norm(sub(c.origin, p)) < 1e-9 || norm(sub(d.origin, p)) < 1e-9));
            if !shared {
                ends.push((i, at_end, p));
            }
        }
    }
    let [first, second] = ends[..] else {
        return Err(error("@opMoveCurveBoundary: only open chains of edges are supported"));
    };
    // The end that moves is the one nearest the help point (or, flipped, the other).
    let away = |p: occt::Vec3| {
        help.map_or(0.0, |h| {
            occt::vertex(p).ok().and_then(|v| v.distance(h).ok()).map_or(f64::INFINITY, |d| d.distance)
        })
    };
    let (index, at_end, _) = if (away(second.2) <= away(first.2)) != flip { second } else { first };
    let edge = &edges[index];
    let (start, end) = points[index];
    let circle = edge.curve_type().ok() == Some(occt::CurveType::Circle);

    let as_cubic = |edge: &Shape, _: &occt::Axis, _: &occt::Axis| edge_as_cubic(edge);
    // A straight limit curve stands for the plane through it square to the curve's plane (an arc trimmed by a line,
    // batch 038).
    let limit = match (limit, limit_curve) {
        (Some(p), _) => Some(p),
        (None, Some(line)) => {
            let (a, b) = (
                line.edge_tangent(0.0, false).map_err(occt_error)?.origin,
                line.edge_tangent(1.0, false).map_err(occt_error)?.origin,
            );
            let along = normalize(sub(b, a)).ok_or_else(|| regen_error("TRIM_CURVE_FAILED"))?;
            let curve_normal = if circle {
                edge.curve_definition().map_err(occt_error)?.frame.normal
            } else {
                as_cubic(edge, &start, &end)?.plane_normal().ok_or_else(|| regen_error("TRIM_CURVE_FAILED"))?
            };
            let normal = normalize(cross(along, curve_normal)).ok_or_else(|| regen_error("TRIM_CURVE_FAILED"))?;
            Some(occt::Frame { origin: a, normal, x: along })
        }
        (None, None) => None,
    };

    if trim {
        // A curve trimmed by a plane: cut where it crosses the plane, keeping the side the help point is on. An arc
        // stays an arc; a B-spline becomes the same B-spline restricted (batch 038).
        let ([_], Some(plane)) = (edges, limit) else {
            return Err(error(
                "@opMoveCurveBoundary: of curves other than lines, only single curves trimmed by planes or lines are supported yet",
            ));
        };
        if !circle {
            let cubic = as_cubic(edge, &start, &end)?;
            let (a, b) = (cubic.start(), cubic.end());
            let cut = cubic
                .plane_crossings(plane.origin, plane.normal)
                .into_iter()
                .find(|t| *t > a + 1e-9 && *t < b - 1e-9)
                .ok_or_else(|| regen_error("TRIM_CURVE_FAILED"))?;
            let kept = if at_end { cubic.segment(cut, b)? } else { cubic.segment(a, cut)? };
            return kept.edge();
        }
        let centre = edge.curve_definition().map_err(occt_error)?.frame.origin;
        let (x, y, radius, angle) = arc_frame(centre, start.origin, start.direction, end.origin)?;
        let k = dot(sub(centre, plane.origin), plane.normal);
        let (a, b) = (radius * dot(x, plane.normal), radius * dot(y, plane.normal));
        let r = a.hypot(b);
        if r < 1e-12 || k.abs() > r {
            return Err(regen_error("TRIM_CURVE_FAILED"));
        }
        let (base, spread) = (b.atan2(a), (-k / r).acos());
        let tau = 2.0 * std::f64::consts::PI;
        let cut = [base - spread, base + spread]
            .into_iter()
            .map(|t| t.rem_euclid(tau))
            .find(|t| *t > 1e-9 && *t < angle - 1e-9)
            .ok_or_else(|| regen_error("TRIM_CURVE_FAILED"))?;
        let (lo, hi) = if at_end { (cut, angle) } else { (0.0, cut) };
        let frame = occt::Frame { origin: centre, normal: cross(x, y), x };
        return occt::arc_edge(frame, radius, lo, hi).map_err(occt_error);
    }

    // How far the extension runs straight from `p` along `d`: the distance left, or up to the limit.
    let straight = |p: occt::Vec3, d: occt::Vec3, already: f64| -> R<f64> {
        let length = if let Some(plane) = limit {
            let rate = dot(d, plane.normal);
            if rate.abs() < 1e-12 {
                return Err(regen_error("TRIM_CURVE_FAILED"));
            }
            dot(sub(plane.origin, p), plane.normal) / rate
        } else if let Some(surface) = limit_surface {
            surface
                .line_crossings(p, d)
                .map_err(occt_error)?
                .into_iter()
                .filter(|t| *t > 1e-9)
                .min_by(f64::total_cmp)
                .ok_or_else(|| regen_error("TRIM_CURVE_FAILED"))?
        } else {
            distance - already
        };
        if length <= 1e-12 {
            return Err(error("@opMoveCurveBoundary: extensions ending within their blend aren't supported yet"));
        }
        Ok(length)
    };
    let moved = if edge.curve_type().ok() == Some(occt::CurveType::Line) {
        let (p, d, fixed) = if at_end {
            (end.origin, end.direction, start.origin)
        } else {
            (start.origin, scale(start.direction, -1.0), end.origin)
        };
        let to = add(p, scale(d, straight(p, d, 0.0)?));
        if at_end { occt::line_edge(fixed, to) } else { occt::line_edge(to, fixed) }.map_err(occt_error)?
    } else {
        // At its start, the curve is extended as its reverse is at its end, its own parameters kept (batch 038).
        let forward = as_cubic(edge, &start, &end)?;
        let cubic = if at_end { forward } else { forward.reversed() };
        let (base, already) = match if curving { cubic.blended()? } else { None } {
            Some(blended) => {
                let length = blended.length(cubic.end(), blended.end());
                (blended, length)
            }
            None => (cubic, 0.0),
        };
        let (p, d) = base.end_ray()?;
        let extended = base.straight_on(straight(p, d, already)?)?;
        if at_end { extended } else { extended.reversed() }.edge()?
    };
    if edges.len() == 1 {
        return Ok(moved);
    }
    let mut all = edges.to_vec();
    all[index] = moved;
    let wires = occt::connect_edges(&all).map_err(occt_error)?;
    let [wire] = wires.as_slice() else { return Err(regen_error("TRIM_CURVE_FAILED")) };
    Ok(wire.clone())
}

/// `@opWrap`: planar faces wrapped onto a cylinder or cone face, as sheets (`SIMPLE` or `TRIM`). The source plane's
/// anchor and direction go to the destination's, without stretching.
pub fn wrap(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let imprint = match field(definition, "wrapType").as_str() {
        Some("SIMPLE") | Some("TRIM") | None => false,
        Some("IMPRINT") => true,
        Some(other) => return Err(error(format!("@opWrap: wrapType {other} isn't supported yet"))),
    };
    let mut destination_face: Option<crate::model::EntityRef> = None;
    let faces = shapes(model, field(definition, "entities"), EntityType::Face)?;
    let source = field(definition, "source");
    let destination = field(definition, "destination");
    // Unwrapping: faces of a cylinder laid onto a plane (batch 046).
    if field(source, "plane").is_undefined() && !field(destination, "plane").is_undefined() {
        return unwrap(model, id, definition, &faces);
    }
    let plane = values::plane(field(source, "plane"), "opWrap source plane")
        .map_err(|_| error("@opWrap: only planar sources are supported yet"))?;
    let anchor = |w: &Value, what: &str| -> R<(occt::Vec3, occt::Vec3)> {
        Ok((
            values::vec3(field(w, "anchorPoint"), what)?,
            values::normalize(values::vec3(field(w, "anchorDirection"), what)?)
                .ok_or_else(|| error("zero direction"))?,
        ))
    };
    let (source_anchor, source_direction) = anchor(source, "opWrap source anchor")?;
    let (destination_anchor, destination_direction) = anchor(destination, "opWrap destination anchor")?;
    // Distances round the destination taken at another radius (a sheet metal bend's allowance).
    let arc_radius = match field(destination, "allowanceRadius") {
        v if v.is_undefined() => None,
        v => Some(values::length(v, "opWrap allowance radius")?),
    };
    // The destination: a face, or a cylinder given as std's `Cylinder` (a sheet metal bend's).
    let target = match field(destination, "cylinder") {
        cylinder if !cylinder.is_undefined() => {
            let cs = field(cylinder, "coordSystem");
            let frame = occt::Frame {
                origin: values::vec3(field(cs, "origin"), "opWrap cylinder origin")?,
                normal: values::normalize(values::vec3(field(cs, "zAxis"), "opWrap cylinder axis")?)
                    .ok_or_else(|| error("zero axis"))?,
                x: values::normalize(values::vec3(field(cs, "xAxis"), "opWrap cylinder x")?)
                    .ok_or_else(|| error("zero x"))?,
            };
            let radius = values::length(field(cylinder, "radius"), "opWrap cylinder radius")?;
            occt::cylinder_face(frame, radius, 1.0).map_err(occt_error)?
        }
        _ => {
            let r = query::evaluate(model, field(destination, "face"))?
                .into_iter()
                .find(|r| r.kind == EntityType::Face)
                .ok_or_else(|| error("@opWrap: the destination must be a face or a cylinder for now"))?;
            destination_face = Some(r);
            model.shape(r).clone()
        }
    };
    // The wrapped faces and edges are tracked back to those they were wrapped from.
    let sources: Vec<crate::model::Entity> = model
        .bodies
        .iter()
        .flat_map(|b| b.faces.iter().chain(b.edges.iter()))
        .filter(|e| {
            faces
                .iter()
                .any(|f| f.is_same(&e.shape) || f.sub_shapes(ShapeKind::Edge).iter().any(|x| x.is_same(&e.shape)))
        })
        .cloned()
        .collect();
    let wrapped = occt::wrap_onto_cylinder(
        &faces,
        (source_anchor, source_direction, plane.normal),
        &target,
        destination_anchor,
        destination_direction,
        arc_radius,
    )
    .map_err(|_| regen_error("WRAP_FAILED"))?;
    // An imprint splits the destination face along the wrapped faces' edges (batch 020).
    if imprint {
        let r = destination_face.ok_or_else(|| error("@opWrap: an imprint needs a destination face"))?;
        let edges = wrapped.shape().sub_shapes(ShapeKind::Edge);
        let op = occt::split_face_by_edges(&model.bodies[r.body].shape, model.shape(r), &edges)
            .map_err(|_| regen_error("WRAP_FAILED"))?;
        super::replace_bodies(model, &id, vec![(r.body, op)]);
        return Ok(Value::UNDEFINED);
    }
    let inputs: Vec<&crate::model::Entity> = sources.iter().collect();
    let origins = Origins::derived(&wrapped, &inputs);
    let info = Info::new(id.clone());
    for face in wrapped.shape().sub_shapes(ShapeKind::Face) {
        model.add_body(NewBody::new(face, BodyType::Sheet, info.clone()), &origins, &info, &|_| None);
    }
    Ok(Value::UNDEFINED)
}

/// `@opWrap` unwrapping: faces of a cylinder (the source's face) laid onto the destination's plane without
/// stretching (batch 046). A face all the way round fails (`ROLL_FAILED`), as in Onshape.
fn unwrap(model: &mut Model, id: FeatureId, definition: &Value, faces: &[Shape]) -> R<Value> {
    let source = field(definition, "source");
    let destination = field(definition, "destination");
    let anchor = |w: &Value, what: &str| -> R<(occt::Vec3, occt::Vec3)> {
        Ok((
            values::vec3(field(w, "anchorPoint"), what)?,
            values::normalize(values::vec3(field(w, "anchorDirection"), what)?)
                .ok_or_else(|| error("zero direction"))?,
        ))
    };
    let source_face = query::evaluate(model, field(source, "face"))?
        .into_iter()
        .find(|r| r.kind == EntityType::Face)
        .map(|r| model.shape(r).clone())
        .ok_or_else(|| error("@opWrap: unwrapping needs the source given as a cylindrical face for now"))?;
    let plane = values::plane(field(destination, "plane"), "opWrap destination plane")?;
    let (destination_anchor, destination_direction) = anchor(destination, "opWrap destination anchor")?;
    let unwrapped = occt::unwrap_from_cylinder(
        faces,
        &source_face,
        anchor(source, "opWrap source anchor")?,
        (destination_anchor, destination_direction, plane.normal),
    )
    .map_err(|_| regen_error("ROLL_FAILED"))?;
    let info = Info::new(id.clone());
    for face in unwrapped.shape().sub_shapes(ShapeKind::Face) {
        model.add_body(NewBody::new(face, BodyType::Sheet, info.clone()), &Origins::default(), &info, &|_| None);
    }
    Ok(Value::UNDEFINED)
}

/// `@opConstrainedSurface`: a smooth sheet within `tolerance` of the points (see [`crate::bspline::fit_surface`]).
/// Normals at the points aren't supported yet.
pub fn constrained_surface(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let entries = field(definition, "points")
        .as_array()
        .ok_or_else(|| error("@opConstrainedSurface: points must be an array"))?;
    if entries.iter().any(|e| !field(e, "normal").is_undefined()) {
        return Err(error("@opConstrainedSurface: normals at the points aren't supported yet"));
    }
    let points =
        entries.iter().map(|e| values::vec3(field(e, "point"), "constrained surface point")).collect::<R<Vec<_>>>()?;
    let tolerance = match field(definition, "tolerance") {
        v if v.is_undefined() => 1e-6,
        v => values::length(v, "constrained surface tolerance")?,
    };
    let smooth = values::opt_bool(definition, "smooth")?.unwrap_or(false);
    let fit = crate::bspline::fit_surface(&points, tolerance, smooth)?;
    let face =
        occt::bspline_surface_face(&fit.poles, fit.counts, &fit.knots_u, &fit.knots_v, (3, 3)).map_err(occt_error)?;
    add(model, &id, face, BodyType::Sheet);
    Ok(Value::UNDEFINED)
}

/// A ruled surface's override at a vertex of its path (std's `vertexOverrides`): its ruling's direction (or angle from
/// the reference faces) and width, or a point it reaches to.
struct VertexOverride {
    vertex: occt::Vec3,
    width: Option<f64>,
    angle: Option<f64>,
    direction: Option<occt::Vec3>,
    up_to: Option<occt::Vec3>,
}

fn vertex_overrides(model: &Model, definition: &Value) -> R<Vec<VertexOverride>> {
    let mut out = Vec::new();
    let Some(list) = field(definition, "vertexOverrides").as_array() else { return Ok(out) };
    let point_of = |q: &Value| -> R<Option<occt::Vec3>> {
        let found = query::evaluate(model, q)?;
        let Some(r) = found.first() else { return Ok(None) };
        let vertices = model.shape(*r).sub_shapes(ShapeKind::Vertex);
        if r.kind != EntityType::Vertex && r.kind != EntityType::Body {
            return Err(error("@opRuledSurface: overrides up to faces or edges aren't supported yet"));
        }
        Ok(vertices.first().and_then(|v| v.vertex_point().ok()))
    };
    for o in list.iter() {
        let Some(vertex) = point_of(field(o, "vertex"))? else { continue };
        let width = match field(o, "width") {
            v if v.is_undefined() => None,
            v => Some(values::length(v, "vertex override width")?),
        };
        let angle = match field(o, "angle") {
            v if v.is_undefined() => None,
            v => Some(values::angle(v, "vertex override angle")?),
        };
        let direction = match field(o, "ruledDirection") {
            v if v.is_undefined() => None,
            v => values::normalize(values::vec3(v, "vertex override direction")?),
        };
        let up_to = match field(o, "upToEntity") {
            v if v.is_undefined() => None,
            v => point_of(v)?,
        };
        out.push(VertexOverride { vertex, width, angle, direction, up_to });
    }
    Ok(out)
}
