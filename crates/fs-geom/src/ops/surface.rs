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
        let shape = match extend_cylindrical(&model.bodies[b], &edges, distance)? {
            Some(patch) => patch,
            None => occt::extend_planar_sheet(&model.bodies[b].shape, &edges, distance)
                .map_err(|_| regen_error("EXTEND_SHEET_BODY_FAILED"))?,
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
    let target_refs: Vec<crate::model::EntityRef> =
        query::evaluate(model, field(definition, "targets"))?.into_iter().filter(|r| r.kind == EntityType::Face).collect();
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
    let direction_of = |edge: &Shape, angle: f64| -> R<occt::Vec3> {
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
                let tangent = used.edge_tangent(0.5, true).map_err(occt_error)?;
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
                let tangent = edge.edge_tangent(0.5, true).map_err(occt_error)?;
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
    let overrides = vertex_overrides(model, definition)?;
    let cubic = values::opt_bool(definition, "useCubicInterpolation")?.unwrap_or(true);
    let mut sheets = Vec::new();
    for edge in &path {
        let direction = direction_of(edge, angle)?;
        let ruling = values::scale(direction, width);
        // The ruling at each end: an override there (batch 024), or the default.
        let ends = [0.0, 1.0].map(|t| edge.edge_tangent(t, false).map(|l| l.origin));
        let (a, b) = (ends[0].clone().map_err(occt_error)?, ends[1].clone().map_err(occt_error)?);
        let at = |p: occt::Vec3| -> R<Option<occt::Vec3>> {
            let Some(o) = overrides.iter().find(|o| values::norm(values::sub(o.vertex, p)) < 1e-7) else { return Ok(None) };
            Ok(Some(match (&o.up_to, o.direction, o.angle) {
                (Some(q), _, _) => values::sub(*q, p),
                (None, Some(d), _) => values::scale(d, o.width.unwrap_or(width)),
                (None, None, Some(a)) => values::scale(direction_of(edge, a)?, o.width.unwrap_or(width)),
                (None, None, None) => values::scale(direction, o.width.unwrap_or(width)),
            }))
        };
        let (start, end) = (at(a)?, at(b)?);
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
        let reference = reference.sub_shapes(ShapeKind::Face).into_iter().next().ok_or_else(|| regen_error("RULED_SURFACE_FAILED"))?;
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
        let face = occt::bspline_surface_face(&poles, (4, 2), &[0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0], &[0.0, 0.0, 1.0, 1.0], (3, 1))
            .map_err(|_| regen_error("RULED_SURFACE_FAILED"))?;
        sheets.push(if faces_along(&face)? { face } else { face.reversed() });
    }
    let joined = occt::extract_faces(&sheets, 0.0).map_err(occt_error)?;
    for sheet in joined {
        add(model, &id, sheet, BodyType::Sheet);
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
            Some(face) => {
                Some(face.face_plane().map_err(|_| error("@opMoveCurveBoundary: only planar limits are supported yet"))?)
            }
            None if limit_curve.is_some() => None,
            None => return Err(regen_error("CANNOT_RESOLVE_ENTITIES")),
        }
    } else {
        limit_curve = None;
        None
    };
    let distance = match (limit, &limit_curve) {
        (None, None) => values::length(field(definition, "extensionDistance"), "extensionDistance")?,
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
        let [edge] = edges.as_slice() else {
            return Err(error("@opMoveCurveBoundary: only single-edge curves are supported yet"));
        };
        if edge.curve_type().ok() != Some(occt::CurveType::Line) {
            return Err(error("@opMoveCurveBoundary: only straight curves are supported yet"));
        }
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
                let d = values::normalize(values::sub(end.origin, start.origin)).ok_or_else(|| regen_error("TRIM_CURVE_FAILED"))?;
                let shared = values::cross(along, d);
                let normal = values::normalize(values::cross(shared, along)).ok_or_else(|| regen_error("TRIM_CURVE_FAILED"))?;
                Some(occt::Frame { origin: c0, normal, x: along })
            }
            _ => None,
        };
        let (a, b2) = if let Some(plane) = limit {
            // Where the line meets the plane.
            let d = values::sub(end.origin, start.origin);
            let rate = values::dot(d, plane.normal);
            if rate.abs() < 1e-12 {
                return Err(regen_error("TRIM_CURVE_FAILED"));
            }
            let t = values::dot(values::sub(plane.origin, start.origin), plane.normal) / rate;
            let hit = values::add(start.origin, values::scale(d, t));
            match (trim, near_end) {
                // Trimming keeps the side the help point is on; extending moves the end nearest it.
                (true, true) | (false, true) => (if trim { hit } else { start.origin }, if trim { end.origin } else { hit }),
                (true, false) | (false, false) => (if trim { start.origin } else { hit }, if trim { hit } else { end.origin }),
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
