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
        let shape = occt::extend_planar_sheet(&model.bodies[b].shape, &edges, distance)
            .map_err(|_| regen_error("EXTEND_SHEET_BODY_FAILED"))?;
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

pub fn curves_on_face(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let groups = field(definition, "curveDefinition")
        .as_array()
        .ok_or_else(|| error("@opCreateCurvesOnFace: curveDefinition must be an array"))?
        .clone();
    for group in groups.iter() {
        let faces = shapes(model, field(group, "face"), EntityType::Face)?;
        let face = faces.first().ok_or_else(|| regen_error("CANNOT_RESOLVE_ENTITIES"))?.clone();
        let creation = field(group, "creationType").as_str().unwrap_or_default().to_string();
        // DIR1 curves run along the face's v parameter at fixed u (on a cylinder, lines along it), DIR2 the other
        // way. (Batch 005, c007 and c008.)
        let along_u = !creation.starts_with("DIR1");
        // Equally spaced curves: across an open range at i / (n - 1), ends included (curves on the face's boundary
        // are then dropped); round a periodic one at (i + ½) / n. (Batch 005, c007 and c008.)
        let auto = creation.contains("AUTO_SPACED");
        let parameters: Vec<f64> = if auto {
            let n = field(group, "nCurves").as_number().unwrap_or(1.0).max(1.0) as usize;
            if occt::face_periodic(&face, !along_u) {
                (0..n).map(|i| (i as f64 + 0.5) / n as f64).collect()
            } else if n == 1 {
                vec![0.5]
            } else {
                (0..n).map(|i| i as f64 / (n - 1) as f64).collect()
            }
        } else {
            values::numbers(field(group, "parameters"), "parameters")?
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
    let edges = shapes(model, field(definition, "edges"), EntityType::Edge)?;
    let targets = shapes(model, field(definition, "targets"), EntityType::Face)?;
    let mut made = Vec::new();
    for edge in &edges {
        let face = targets
            .iter()
            .find(|f| f.sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(edge)))
            .ok_or_else(|| regen_error("OFFSET_CURVE_ON_FACE_INVALID_TARGETS"))?;
        let signed = if opposite { -distance } else { distance };
        made.push(
            occt::offset_edge_on_planar_face(edge, face, signed)
                .map_err(|_| regen_error("OFFSET_CURVE_ON_FACE_FAILED"))?,
        );
    }
    add_wires(model, &id, &made)?;
    Ok(Value::UNDEFINED)
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
    let kind = field(definition, "ruledSurfaceType").as_str().unwrap_or("ANGLE_FROM_FACE").to_string();
    let path = shapes(model, field(definition, "path"), EntityType::Edge)?;
    if path.is_empty() {
        return Err(regen_error("RULED_SURFACE_SELECT_EDGES"));
    }
    let references = match field(definition, "referenceFaces") {
        v if v.is_undefined() => Vec::new(),
        v => faces_of(model, v)?,
    };
    let mut sheets = Vec::new();
    for edge in &path {
        let direction = match kind.as_str() {
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
            other => return Err(error(format!("@opRuledSurface: {other} isn't supported yet"))),
        };
        let op = occt::prism(edge, values::scale(direction, width)).map_err(|_| regen_error("RULED_SURFACE_FAILED"))?;
        sheets.extend(op.shape().sub_shapes(ShapeKind::Face).into_iter().map(|f| f.reversed()));
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
    if field(definition, "moveBoundaryType").as_str() != Some("EXTEND")
        || field(definition, "endCondition").as_str().is_some_and(|c| c != "BLIND")
    {
        return Err(error("@opMoveCurveBoundary: only extending by a distance is supported yet"));
    }
    let distance = values::length(field(definition, "extensionDistance"), "extensionDistance")?;
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
        let (a, b2) = if near_end {
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

/// `@opWrap`: planar faces wrapped onto a cylinder face, as sheets (`SIMPLE` or `TRIM`). The source plane's anchor
/// and direction go to the destination's, without stretching.
pub fn wrap(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    match field(definition, "wrapType").as_str() {
        Some("SIMPLE") | Some("TRIM") | None => {}
        Some(other) => return Err(error(format!("@opWrap: wrapType {other} isn't supported yet"))),
    }
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
