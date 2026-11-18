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
    let offset = match field(definition, "offset") {
        v if v.is_undefined() => 0.0,
        v => values::length(v, "opExtractSurface offset")?,
    };
    let mut faces = faces_of(model, field(definition, "faces"))?;
    // With tangent propagation, the faces tangent to them across their edges too (sheet metal's thicken start,
    // batch 164 c006).
    if values::opt_bool(definition, "tangentPropagation")?.unwrap_or(false) {
        let seeds: Vec<crate::model::EntityRef> =
            query::evaluate(model, field(definition, "faces"))?.into_iter().filter(|r| r.kind == EntityType::Face).collect();
        for r in crate::topology::tangent_connected_faces(model, &seeds, crate::topology::TANGENT_TOLERANCE) {
            let shape = model.shape(r).clone();
            if !faces.iter().any(|f| f.is_same(&shape)) {
                faces.push(shape);
            }
        }
    }
    if faces.is_empty() {
        return Err(regen_error("EXTRACT_SURFACE_FAILED"));
    }
    // The copies' entities are tracked back to the faces, edges and vertices they lie on (std tracks a sketch's
    // lines into the surfaces extracted from its regions).
    let mut sources: Vec<crate::model::Entity> = Vec::new();
    {
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
    // An offset copy's edges run as the edges they're offset from, as Onshape's (batch 086 c004, 087 c003): each
    // source edge as Onshape's runs.
    let mut source_edges: Vec<Shape> = Vec::new();
    if offset != 0.0 {
        for (b, body) in model.bodies.iter().enumerate() {
            for (index, edge) in body.edges.iter().enumerate() {
                if faces.iter().any(|f| f.sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(&edge.shape))) {
                    let r = crate::model::EntityRef { body: b, kind: EntityType::Edge, index };
                    source_edges.push(super::onshape_edge(model, r));
                }
            }
        }
    }
    let sense_of = |copy: &Shape| -> Option<(occt::Vec3, occt::Vec3)> {
        let mid = copy.edge_tangent(0.5, false).ok()?;
        let p = occt::vertex(mid.origin).ok()?;
        let (source, _) = source_edges
            .iter()
            .filter_map(|e| e.distance(&p).ok().map(|d| (e, d.distance)))
            .filter(|(_, d)| (d - offset.abs()).abs() < 1e-6)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))?;
        let along = source.edge_tangent(source.edge_fraction(mid.origin, false).ok()?, false).ok()?;
        (values::dot(along.direction, mid.direction).abs() > 1.0 - 1e-6).then_some((mid.origin, along.direction))
    };
    for sheet in occt::extract_faces(&faces, offset).map_err(|_| regen_error("EXTRACT_SURFACE_FAILED"))? {
        let mut origins = Origins::default();
        if offset == 0.0 {
            for kind in [ShapeKind::Face, ShapeKind::Edge, ShapeKind::Vertex] {
                for copy in sheet.sub_shapes(kind) {
                    for source in sources.iter().filter(|e| e.shape.kind() == Some(kind)) {
                        if coincide(&copy, &source.shape) {
                            origins.derive(&copy, source, true);
                        }
                    }
                }
            }
        } else {
            derive_offset_copies(&sheet, &sources, offset.abs(), &mut origins);
        }
        let info = Info::new(id.clone());
        let special = |s: &Shape| {
            if s.kind() != Some(ShapeKind::Edge) || source_edges.is_empty() {
                return None;
            }
            sense_of(s).map(|sense| std::rc::Rc::new(Info { sense: Some(sense), ..(*info).clone() }))
        };
        model.add_body(NewBody::new(sheet, BodyType::Sheet, info.clone()), &origins, &info, &special);
    }
    Ok(Value::UNDEFINED)
}

/// Tracks an offset copy's entities back to what they were offset from (sheet metal's conversion with a clearance
/// tracks its bend edges so, batch 105 c007): each face to the source face it lies `offset` from, each edge and
/// vertex to the source entity bounded by the faces its own faces came from, the nearest if several are.
fn derive_offset_copies(sheet: &Shape, sources: &[crate::model::Entity], offset: f64, origins: &mut Origins) {
    // Each source entity once (a face can be listed by more than one body's entities).
    // A copy comes from every entity that is the source's shape (bodies can share one).
    let derive_all = |origins: &mut Origins, copy: &Shape, source: &Shape| {
        for e in sources.iter().filter(|e| e.shape.is_same(source)) {
            origins.derive(copy, e, true);
        }
    };
    let of_kind = |kind| {
        let mut out: Vec<&crate::model::Entity> = Vec::new();
        for e in sources.iter().filter(|e| e.shape.kind() == Some(kind)) {
            if !out.iter().any(|o| o.shape.is_same(&e.shape)) {
                out.push(e);
            }
        }
        out
    };
    let source_faces = of_kind(ShapeKind::Face);
    let copy_faces = sheet.sub_shapes(ShapeKind::Face);
    // Each copy face's source face, by index into `source_faces`.
    let mut face_source: Vec<Option<usize>> = Vec::new();
    for copy in &copy_faces {
        let found = copy.face_inner_point().ok().and_then(|p| {
            let probe = occt::vertex(p).ok()?;
            source_faces
                .iter()
                .enumerate()
                .filter_map(|(i, f)| f.shape.distance(&probe).ok().map(|d| (i, (d.distance - offset).abs())))
                .filter(|(_, miss)| *miss < 1e-6)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i)
        });
        if let Some(i) = found {
            derive_all(origins, copy, &source_faces[i].shape);
        }
        face_source.push(found);
    }
    // The source faces an entity bounds, and the copy faces' sources an entity of the copy bounds.
    let bounded_by = |faces: &[Shape], shape: &Shape, kind: ShapeKind| -> Vec<usize> {
        (0..faces.len()).filter(|&i| faces[i].sub_shapes(kind).iter().any(|x| x.is_same(shape))).collect()
    };
    let source_face_shapes: Vec<Shape> = source_faces.iter().map(|f| f.shape.clone()).collect();
    let point_of = |s: &Shape| match s.kind() {
        Some(ShapeKind::Vertex) => s.vertex_point().ok(),
        _ => s.edge_tangent(0.5, false).ok().map(|a| a.origin),
    };
    for kind in [ShapeKind::Edge, ShapeKind::Vertex] {
        let candidates = of_kind(kind);
        let source_sets: Vec<Vec<usize>> =
            candidates.iter().map(|e| bounded_by(&source_face_shapes, &e.shape, kind)).collect();
        for copy in sheet.sub_shapes(kind) {
            let mut wanted: Vec<usize> =
                bounded_by(&copy_faces, &copy, kind).into_iter().filter_map(|i| face_source[i]).collect();
            wanted.sort_unstable();
            wanted.dedup();
            if wanted.is_empty() {
                continue;
            }
            let Some(at) = point_of(&copy).and_then(|p| occt::vertex(p).ok()) else { continue };
            let best = candidates
                .iter()
                .zip(&source_sets)
                .filter(|(_, set)| **set == wanted)
                .filter_map(|(e, _)| e.shape.distance(&at).ok().map(|d| (*e, d.distance)))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((source, _)) = best {
                derive_all(origins, &copy, &source.shape);
            }
        }
    }
}

pub fn intersect_faces(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let tools = faces_of(model, field(definition, "tools"))?;
    let targets = faces_of(model, field(definition, "targets"))?;
    let edges = occt::section_edges(&tools, &targets).map_err(|_| regen_error("INTERSECTION_CURVE_FAILED"))?;
    if edges.is_empty() {
        return Err(regen_error("INTERSECTION_CURVE_NO_INTERSECTION"));
    }
    // Each curve joined into one edge, then split where Onshape splits it.
    let mut curves: Vec<(Shape, f64)> = Vec::new();
    for wire in occt::connect_edges(&edges).map_err(occt_error)? {
        let joined =
            if wire.sub_shapes(ShapeKind::Edge).len() > 1 { occt::join_edges(&wire).unwrap_or(wire) } else { wire };
        let single = joined.kind() == Some(ShapeKind::Edge) || joined.sub_shapes(ShapeKind::Edge).len() == 1;
        let edge = if single { joined.sub_shapes(ShapeKind::Edge).into_iter().next() } else { None };
        match edge.and_then(|e| super::intersection::split_curve(model, &e, &tools, &targets)) {
            Some((pieces, key)) => {
                curves.push((occt::ordered_wire(&pieces).map_err(occt_error)?, key));
            }
            None => curves.push((joined, f64::MAX)),
        }
    }
    curves.sort_by(|a, b| a.1.total_cmp(&b.1));
    for (curve, _) in curves {
        add(model, &id, curve, BodyType::Wire);
    }
    Ok(Value::UNDEFINED)
}

pub fn fill_surface(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let found = |key: &str| -> R<Vec<crate::model::EntityRef>> {
        if field(definition, key).is_undefined() { Ok(Vec::new()) } else { query::evaluate(model, field(definition, key)) }
    };
    // Guides (std's fill passes `guideEntities`; the operation documents `guideVertices`), checked below.
    let mut guides = found("guideVertices")?;
    guides.extend(found("guideEntities")?);
    // Each edge as Onshape's runs: a flat fill of sketch curves faces as the loop of them runs, from the first (batch
    // 158; 159 c009–c011: drawn half each way round, whichever is listed first sets it).
    let mut edges = Vec::new();
    let mut refs = Vec::new();
    for r in query::evaluate(model, field(definition, "edgesG0"))? {
        match r.kind {
            EntityType::Edge => {
                refs.push(r);
                edges.push(super::onshape_edge(model, r));
            }
            EntityType::Body => edges.extend(model.body(r).edges.iter().map(|e| e.shape.clone())),
            _ => {}
        }
    }
    // Edges the fill is to be tangent along: where the loop is flat and the faces along them lie in its plane, the flat
    // fill already is, and Onshape's is that plane (batch 157 c003: a gap whose far sheet rises as the gap does).
    // Curvature continuity along a flat face the flat fill continues is met as tangency is (a plane's curvature is
    // a plane's).
    let mut tangent = found("edgesG1")?;
    tangent.extend(found("edgesG2")?);
    if !tangent.is_empty() {
        let mut all = edges.clone();
        all.extend(tangent.iter().filter(|r| r.kind == EntityType::Edge).map(|&r| super::onshape_edge(model, r)));
        let normal = loop_plane(&all);
        let continued = tangent.iter().all(|&r| {
            let edge = model.shape(r).clone();
            let faces: Vec<&Shape> = model.bodies[r.body]
                .faces
                .iter()
                .map(|f| &f.shape)
                .filter(|f| f.sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(&edge)))
                .collect();
            r.kind == EntityType::Edge
                && !faces.is_empty()
                && normal.is_some_and(|n| {
                    faces.iter().all(|f| {
                        // (Within 1e-5 of a radian: batch 157's rising sheet lies 4.5e-7 off the gap's plane, its
                        // sketch plane's normal given to six places.)
                        f.face_plane().is_ok_and(|p| values::norm(values::cross(p.normal, n)) < 1e-5)
                    })
                })
        });
        if !continued {
            return Err(error("@opFillSurface: tangent and curvature conditions other than along faces the flat fill continues aren't supported yet"));
        }
        // In the order std lists the edges: the G0 ones, then the G1 ones.
        edges = all;
        refs.extend(tangent.iter().filter(|r| r.kind == EntityType::Edge));
    }
    // Guides the flat fill passes through anyway change nothing; others aren't supported yet.
    if !guides.is_empty() {
        let plane = loop_plane(&edges);
        let origin = edges.first().and_then(|e| e.edge_tangent(0.0, false).ok()).map(|t| t.origin);
        let on = |p: occt::Vec3| match (plane, origin) {
            (Some(n), Some(o)) => values::dot(values::sub(p, o), n).abs() < 1e-7,
            _ => false,
        };
        let flat = guides.iter().all(|&r| {
            let shape = model.shape(r).clone();
            match r.kind {
                EntityType::Vertex => shape.vertex_point().is_ok_and(on),
                EntityType::Edge => (0..=8).all(|k| shape.edge_tangent(k as f64 / 8.0, false).is_ok_and(|t| on(t.origin))),
                _ => false,
            }
        });
        if !flat {
            return Err(error("@opFillSurface: guides off the flat fill's plane aren't supported yet"));
        }
    }
    // A flat loop with sheets' edges in it faces as if sewn to the oldest of those sheets: it runs that sheet's edge
    // the other way from the sheet's own loop, whichever edge is listed first (batch 159 c004, c006–c008: a gap
    // between a level sheet and a rising one faces as the level sheet, made first, whatever the order).
    if loop_plane(&edges).is_some() {
        let sheet_edge = refs
            .iter()
            .filter(|r| model.bodies[r.body].body_type == BodyType::Sheet)
            .filter_map(|&r| {
                let edge = model.shape(r).clone();
                let mut runs = model.bodies[r.body]
                    .faces
                    .iter()
                    .filter_map(|f| f.shape.sub_shapes(ShapeKind::Edge).into_iter().find(|e| e.is_same(&edge)));
                match (runs.next(), runs.next()) {
                    (Some(run), None) => Some((r.body, edge, run.reversed())),
                    _ => None,
                }
            })
            .min_by_key(|(body, _, _)| *body);
        if let Some((_, edge, run)) = sheet_edge {
            edges.retain(|e| !e.is_same(&edge));
            edges.insert(0, run);
        }
    }
    let face = match bilinear_fill(&edges).or_else(|| coons_fill(&edges)) {
        Some(face) => face,
        None => occt::fill_edges(&edges).map_err(|_| regen_error("FILL_SURFACE_FAILED"))?,
    };
    add(model, &id, face, BodyType::Sheet);
    Ok(Value::UNDEFINED)
}

/// The normal of the plane a loop of edges lies in, if it lies in one.
fn loop_plane(edges: &[Shape]) -> Option<occt::Vec3> {
    let points: Vec<occt::Vec3> = edges
        .iter()
        .flat_map(|e| (0..=8).filter_map(|k| e.edge_tangent(k as f64 / 8.0, false).ok().map(|t| t.origin)).collect::<Vec<_>>())
        .collect();
    let normal = (0..points.len()).find_map(|i| {
        (i + 1..points.len()).find_map(|j| {
            (j + 1..points.len()).find_map(|k| {
                let n = values::cross(values::sub(points[j], points[i]), values::sub(points[k], points[i]));
                (values::norm(n) > 1e-9).then(|| values::normalize(n)).flatten()
            })
        })
    })?;
    points.iter().all(|p| values::dot(values::sub(*p, points[0]), normal).abs() < 1e-7).then_some(normal)
}

/// Four edges round a loop that isn't flat, not all straight: the Coons patch of the four, as a boundary surface
/// through two pairs of them is (batch 063 c009), the first edge and the one opposite it one pair, facing as the
/// bilinear patch between its corners does.
fn coons_fill(edges: &[Shape]) -> Option<Shape> {
    if edges.len() != 4 || edges.iter().all(|e| e.curve_type().ok() == Some(occt::CurveType::Line)) {
        return None;
    }
    let ends = |e: &Shape| -> Option<(occt::Vec3, occt::Vec3)> {
        Some((e.edge_tangent(0.0, false).ok()?.origin, e.edge_tangent(1.0, false).ok()?.origin))
    };
    let close = |a: occt::Vec3, b: occt::Vec3| values::norm(values::sub(a, b)) < 1e-7;
    let (a0, a1) = ends(&edges[0])?;
    let touches = |e: &Shape, p: occt::Vec3| ends(e).is_some_and(|(x, y)| close(x, p) || close(y, p));
    let opposite = edges[1..].iter().find(|e| !touches(e, a0) && !touches(e, a1))?;
    let sides: Vec<Shape> = edges[1..].iter().filter(|e| !e.is_same(opposite)).cloned().collect();
    let [c, d] = &sides[..] else { return None };
    // A loop lying in one plane is filled flat, as Onshape's planar fills are.
    let points: Vec<occt::Vec3> = edges
        .iter()
        .flat_map(|e| (0..=8).filter_map(|k| e.edge_tangent(k as f64 / 8.0, false).ok().map(|t| t.origin)).collect::<Vec<_>>())
        .collect();
    let normal = (0..points.len()).find_map(|i| {
        (i + 1..points.len()).find_map(|j| {
            (j + 1..points.len()).find_map(|k| {
                let n = values::cross(values::sub(points[j], points[i]), values::sub(points[k], points[i]));
                (values::norm(n) > 1e-9).then(|| values::normalize(n)).flatten()
            })
        })
    })?;
    if points.iter().all(|p| values::dot(values::sub(*p, points[0]), normal).abs() < 1e-9) {
        return None;
    }
    // Facing as the bilinear patch between its corners, even where they lie in a plane (a pillow of arcs over a
    // rectangle's sides, batch 153 c001; a saddle of arcs and lines, c002).
    let reference = bilinear_patch(edges, false, true)?;
    coons_surface(&[edges[0].clone(), opposite.clone()], &[c.clone(), d.clone()], &reference).ok()
}

/// Four straight edges round a loop that isn't flat: Onshape fills it with the bilinear patch between them (batch
/// 062 c004), u running back along the first edge from where it meets the second, v along the second.
fn bilinear_fill(edges: &[Shape]) -> Option<Shape> {
    bilinear_patch(edges, true, false)
}

/// The bilinear patch between the corners of a loop of four edges (all straight, if `straight`), as
/// [`bilinear_fill`] lays it out; `None` if the loop isn't four edges end to end, or its corners lie in a plane
/// (unless `flat`).
fn bilinear_patch(edges: &[Shape], straight: bool, flat: bool) -> Option<Shape> {
    if edges.len() != 4 || (straight && edges.iter().any(|e| e.curve_type().ok() != Some(occt::CurveType::Line))) {
        return None;
    }
    let ends = |e: &Shape| -> Option<(occt::Vec3, occt::Vec3)> {
        Some((e.edge_tangent(0.0, false).ok()?.origin, e.edge_tangent(1.0, false).ok()?.origin))
    };
    let close = |a: occt::Vec3, b: occt::Vec3| values::norm(values::sub(a, b)) < 1e-9;
    let (a, b) = ends(&edges[0])?;
    let (c, d) = ends(&edges[1])?;
    let (origin, u_end, v_end) = if close(b, c) {
        (b, a, d)
    } else if close(b, d) {
        (b, a, c)
    } else if close(a, c) {
        (a, b, d)
    } else if close(a, d) {
        (a, b, c)
    } else {
        return None;
    };
    // The fourth corner: the end of the other edges that is none of the three.
    let corners = [origin, u_end, v_end];
    let opposite = edges[2..]
        .iter()
        .filter_map(ends)
        .flat_map(|(p, q)| [p, q])
        .find(|p| !corners.iter().any(|c| close(*c, *p)))?;
    let normal = values::normalize(values::cross(values::sub(u_end, origin), values::sub(v_end, origin)))?;
    if !flat && values::dot(values::sub(opposite, origin), normal).abs() < 1e-9 {
        return None;
    }
    occt::bspline_surface_face(&[origin, v_end, u_end, opposite], (2, 2), &[0.0, 0.0, 1.0, 1.0], &[0.0, 0.0, 1.0, 1.0], (1, 1))
        .ok()
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
            .or_else(|| extend_revolution(&model.bodies[b], &edges, distance, curving).transpose())
            .transpose()?
        {
            Some(patch) => patch,
            None => match extend_extruded(&model.bodies[b], &edges, distance, curving)?
                .map(Ok)
                .or_else(|| extend_ruled(&model.bodies[b], &edges, distance).transpose())
                .transpose()?
            {
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
        // A sheet lofted through arcs, extended, is still that loft to a face replaced up to it (batch 108 c011).
        let lofted = old.faces.iter().any(|f| f.info.lofted);
        let marked = std::rc::Rc::new(Info { lofted: true, ..(*info).clone() });
        let special = |s: &Shape| (lofted && s.kind() == Some(ShapeKind::Face)).then(|| marked.clone());
        model.add_body(NewBody::replacing(&old, shape), &origins, &info, &special);
    }
    Ok(Value::UNDEFINED)
}

/// A sheet of one face on a cone, sphere or torus, bounded by two generators, meridians or tube circles (a constant
/// u) and two circles round the axis (a constant v), extended as Onshape extends it (batch 039): each point of an
/// edge carried the distance along the surface's circle or line through it, square to the edge. Across a generator
/// or meridian every point goes the distance round its own circle, so the new edge is a curve (d / r at radius r):
/// on a quarter cone, Onshape's new edge reaches exactly 0.5 rad round at radius 10 for 5 mm. Past a circle the
/// edge moves the distance along the generators or meridians. (Onshape's new edges are its own approximations,
/// off this by up to 0.03 mm²; a sphere's extended round strays further, turning 1.106 rad at radius 5 for 5 mm.)
/// A torus turns uniformly round its axis, as far as its innermost circle would for the distance, and goes the
/// distance round its tube (batch 057).
fn extend_revolved(body: &crate::model::Body, edges: &[Shape], distance: f64) -> R<Option<Shape>> {
    use crate::values::{dot, norm, scale, sub};
    let [face] = &body.faces[..] else { return Ok(None) };
    let face = &face.shape;
    let Ok(s) = face.surface_definition() else { return Ok(None) };
    if !matches!(s.kind, 2..=4) {
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
    // Along a cone's generators v is length; along a sphere's meridians, or a torus's tube, angle.
    let dv = match s.kind {
        2 => distance,
        3 => distance / s.radius1,
        _ => distance / s.radius2,
    };
    // Round a torus, every point turns as far as the torus's innermost circle would for the distance (batch 057
    // c011: 5 mm round a torus of radii 10 and 3 turns 5/7 rad all along the tube).
    let torus_turn = if s.kind == 4 { Some(distance / (s.radius1 - s.radius2)) } else { None };
    if torus_turn.is_some_and(|t| !t.is_finite() || t <= 0.0) {
        return Ok(None);
    }
    let (vb, vt) = (if extend[2] { v0 - dv } else { v0 }, if extend[3] { v1 + dv } else { v1 });
    let side = |v: f64, at: f64, out: bool, sign: f64| -> R<f64> {
        if let (Some(turn), true) = (torus_turn, out) {
            return Ok(at + sign * turn);
        }
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
        let n = if out && torus_turn.is_none() { samples } else { 1 };
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

/// A sheet of one face revolved from a cubic curve (batch 079 c008), bounded by two copies of the curve and two
/// circles: past a circle the curve is extended as `trimCurve` extends it (straight on, or keeping its curvature)
/// and revolved again; past a copy of the curve each point goes the distance round its circle, as a cone's does.
fn extend_revolution(body: &crate::model::Body, edges: &[Shape], distance: f64, curving: bool) -> R<Option<Shape>> {
    use crate::values::{dot, norm, scale, sub};
    let [face] = &body.faces[..] else { return Ok(None) };
    let face = &face.shape;
    if face.surface_type().ok() != Some(occt::SurfaceType::Revolution) {
        return Ok(None);
    }
    let failed = || regen_error("EXTEND_SHEET_BODY_FAILED");
    let ((u0, _), (u1, _)) = face.face_uv_bounds().map_err(occt_error)?;
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
    // The axis, from a circle round it; the curve, at the first angle.
    let circle = occt::iso_curves(face, true, 0.5).map_err(occt_error)?.into_iter().next().ok_or_else(failed)?;
    let Ok(c) = circle.curve_definition() else { return Ok(None) };
    let axis = occt::Axis { origin: c.frame.origin, direction: c.frame.normal };
    let meridian = occt::iso_curves(face, false, 0.0).map_err(occt_error)?.into_iter().next().ok_or_else(failed)?;
    let Ok(mut profile) = edge_as_cubic(&meridian) else { return Ok(None) };
    // Which end of the curve is the face's v1.
    let top = face.face_tangent_plane(0.0, 1.0).map_err(|_| failed())?.origin;
    let ends_top = norm(sub(profile.derivative(profile.end(), 0), top)) < 1e-6;
    for (k, at_end) in [(2, !ends_top), (3, ends_top)] {
        if extend[k] {
            profile = extended_cubic(profile, at_end, curving, distance, true)?;
        }
    }
    let radius = |p: occt::Vec3| {
        let d = sub(p, axis.origin);
        norm(sub(d, scale(axis.direction, dot(d, axis.direction))))
    };
    let (start, end) = (profile.knots[0], profile.end());
    let samples = 32;
    let at = |i: usize| start + (end - start) * i as f64 / samples as f64;
    let radii: Vec<f64> = (0..=samples).map(|i| radius(profile.derivative(at(i), 0))).collect();
    if radii.iter().any(|r| *r < 1e-9) {
        return Err(failed());
    }
    let reach = distance / radii.iter().cloned().fold(f64::MAX, f64::min);
    let before = if extend[0] { reach } else { 0.0 };
    let total = (u1 - u0) + before + if extend[1] { reach } else { 0.0 };
    // Revolved from `before` short of the first angle.
    let curve = occt::transform(&profile.edge()?, &super::rotation(axis, u0 - before)).map_err(occt_error)?.shape();
    let swept = occt::revolve(&curve, axis, total).map_err(|_| failed())?.shape();
    let new_face = swept.sub_shapes(ShapeKind::Face).into_iter().next().ok_or_else(failed)?;
    let ((nu0, nv0), (_, nv1)) = new_face.face_uv_bounds().map_err(occt_error)?;
    // The new face's u from the first angle `before` back; its v along the extended curve, either way.
    let (first, last) = (nu0 + before, nu0 + before + (u1 - u0));
    let v_of = |i: usize| nv0 + (nv1 - nv0) * i as f64 / samples as f64;
    let column = |at_u: f64, out: bool, sign: f64, up: bool| -> Vec<(f64, f64)> {
        let mut points: Vec<(f64, f64)> =
            (0..=samples).map(|i| (if out { at_u + sign * distance / radii[i] } else { at_u }, v_of(i))).collect();
        if !out {
            points = vec![points[0], points[samples]];
        }
        if !up {
            points.reverse();
        }
        points
    };
    // Both sides run up the curve, as Onshape's new side does (batch 080 c008).
    let right = column(last, extend[1], 1.0, true);
    let left = column(first, extend[0], -1.0, true);
    let bottom = vec![(left[0].0, nv0), (right[0].0, nv0)];
    let top = vec![(right[right.len() - 1].0, nv1), (left[left.len() - 1].0, nv1)];
    occt::face_from_uv(&new_face, &[bottom, right, top, left]).map(Some).map_err(|_| failed())
}

/// A sheet of one face extruded from a curve, bounded by two copies of the curve and two straight edges (batch 039):
/// at a straight edge the curve is extended as `trimCurve` extends it, straight on or keeping its curvature, and
/// extruded again; at a copy of the curve the extrusion is made longer.
/// A ruled sheet of one face (a loft between two curves) extended past one of its two curved edges: each point of
/// the edge carried the distance on along its ruling, the sheet ruled from the other curve to the new one (batch
/// 065 c004: Onshape extends the surface along its rulings and bounds it there, within 0.001 mm of the distance).
/// Its straight sides keep vertices where the old edge's ends were.
fn extend_ruled(body: &crate::model::Body, edges: &[Shape], distance: f64) -> R<Option<Shape>> {
    if edges.len() > 1 {
        return extend_ruled_all(body, edges, distance);
    }
    Ok(extend_ruled_unsplit(body, edges, distance)?.map(|(sheet, corners)| split_at(sheet, &corners)))
}

/// A ruled sheet's face split where the old corners lie on its boundary (they stay as vertices there).
fn split_at(sheet: Shape, corners: &[occt::Vec3]) -> Shape {
    let edges = sheet.sub_shapes(ShapeKind::Edge);
    let on_boundary: Vec<Shape> = corners
        .iter()
        .filter_map(|&p| occt::vertex(p).ok())
        .filter(|v| edges.iter().any(|e| v.distance(e).is_ok_and(|d| d.distance < 1e-7)))
        .collect();
    if on_boundary.is_empty() {
        return sheet;
    }
    occt::split(&[sheet.clone()], &on_boundary).map(|op| op.shape()).unwrap_or(sheet)
}

/// A ruled sheet extended at several of its edges (batch 108 c011: every edge of a loft between arcs): the curves
/// first, along the rulings, then the straight sides, each from the sheet the last left.
fn extend_ruled_all(body: &crate::model::Body, edges: &[Shape], distance: f64) -> R<Option<Shape>> {
    let [face] = &body.faces[..] else { return Ok(None) };
    let Some(edge_template) = body.edges.first().cloned() else { return Ok(None) };
    let mut order: Vec<&Shape> = edges.iter().filter(|e| !straight_edge(e)).collect();
    order.extend(edges.iter().filter(|e| straight_edge(e)));
    let mut current = body.clone();
    let mut corners: Vec<occt::Vec3> = Vec::new();
    for original in order {
        let middle = original.edge_tangent(0.5, false).map_err(occt_error)?.origin;
        let Ok(probe) = occt::vertex(middle) else { return Ok(None) };
        let Some(edge) = current
            .edges
            .iter()
            .find(|e| e.shape.distance(&probe).is_ok_and(|d| d.distance < 1e-7))
            .map(|e| e.shape.clone())
        else {
            return Ok(None);
        };
        let Some((sheet, old)) = extend_ruled_unsplit(&current, &[edge], distance)? else { return Ok(None) };
        corners.extend(old);
        let entities = |kind: ShapeKind, template: &crate::model::Entity| -> Vec<crate::model::Entity> {
            sheet
                .visible_sub_shapes(kind)
                .into_iter()
                .map(|shape| crate::model::Entity { shape, ..template.clone() })
                .collect()
        };
        current = crate::model::Body {
            faces: entities(ShapeKind::Face, face),
            edges: entities(ShapeKind::Edge, &edge_template),
            vertices: entities(ShapeKind::Vertex, &edge_template),
            shape: sheet.clone(),
            ..current
        };
    }
    Ok(Some(split_at(current.shape, &corners)))
}

/// Whether an edge is straight: a line, or a straight B-spline from a loft.
fn straight_edge(edge: &Shape) -> bool {
    edge.curve_type().ok() == Some(occt::CurveType::Line)
        || [0.25, 0.5, 0.75].iter().all(|&t| {
            let (Ok(a), Ok(b), Ok(m)) =
                (edge.edge_tangent(0.0, false), edge.edge_tangent(1.0, false), edge.edge_tangent(t, false))
            else {
                return false;
            };
            let chord = crate::values::sub(b.origin, a.origin);
            let off = crate::values::sub(m.origin, a.origin);
            let along = crate::values::dot(off, chord) / crate::values::dot(chord, chord);
            crate::values::norm(crate::values::sub(off, crate::values::scale(chord, along))) < 1e-9
        })
}

/// [`extend_ruled`] at one edge, the sheet unsplit, with the old corners.
fn extend_ruled_unsplit(
    body: &crate::model::Body,
    edges: &[Shape],
    distance: f64,
) -> R<Option<(Shape, Vec<occt::Vec3>)>> {
    use crate::values::{add, norm, normalize, scale, sub};
    let failed = || regen_error("EXTEND_SHEET_BODY_FAILED");
    let [face] = &body.faces[..] else { return Ok(None) };
    let [edge] = edges else { return Ok(None) };
    if !matches!(face.shape.surface_type(), Ok(occt::SurfaceType::BSpline | occt::SurfaceType::Bezier)) {
        return Ok(None);
    }
    let own: Vec<Shape> = body.edges.iter().map(|e| e.shape.clone()).collect();
    if own.len() != 4 {
        return Ok(None);
    }
    // A straight side (a line, or a straight B-spline from the loft).
    if straight_edge(edge) {
        return extend_ruled_along(&face.shape, &own, edge, distance);
    }
    let ends = |e: &Shape| -> R<(occt::Vec3, occt::Vec3)> {
        Ok((e.edge_tangent(0.0, false).map_err(occt_error)?.origin, e.edge_tangent(1.0, false).map_err(occt_error)?.origin))
    };
    let close = |a: occt::Vec3, b: occt::Vec3| norm(sub(a, b)) < 1e-7;
    let (e0, e1) = ends(edge)?;
    // The other curve: the edge touching neither of its ends.
    let mut opposite = None;
    for other in &own {
        let (o0, o1) = ends(other)?;
        if ![o0, o1].iter().any(|p| close(*p, e0) || close(*p, e1)) {
            opposite = Some((other.clone(), o0, o1));
        }
    }
    let Some((other, o0, o1)) = opposite else { return Ok(None) };
    // The rulings join the curves' points at the same fraction, the other curve turned to start where the edge's
    // ruling does.
    let reversed = norm(sub(o0, e0)) + norm(sub(o1, e1)) > norm(sub(o1, e0)) + norm(sub(o0, e1));
    const SAMPLES: usize = 32;
    let mut points = Vec::new();
    for i in 0..=SAMPLES {
        let t = i as f64 / SAMPLES as f64;
        let p = edge.edge_tangent(t, false).map_err(occt_error)?.origin;
        let q = other.edge_tangent(if reversed { 1.0 - t } else { t }, false).map_err(occt_error)?.origin;
        // A ruled sheet: halfway along the line, still on the face.
        let middle = scale(add(p, q), 0.5);
        if occt::vertex(middle).and_then(|v| v.distance(&face.shape)).map_or(true, |d| d.distance > 1e-6) {
            return Ok(None);
        }
        let along = normalize(sub(p, q)).ok_or_else(failed)?;
        points.push(add(p, scale(along, distance)));
    }
    let mut parameters = vec![0.0];
    for w in points.windows(2) {
        parameters.push(parameters.last().copied().unwrap_or(0.0) + norm(sub(w[1], w[0])));
    }
    let n = points.len();
    let derivatives: Vec<occt::Vec3> = (0..n)
        .map(|k| {
            let (a, b) = if k == 0 { (0, 1) } else if k == n - 1 { (n - 2, n - 1) } else { (k - 1, k + 1) };
            scale(sub(points[b], points[a]), 1.0 / (parameters[b] - parameters[a]))
        })
        .collect();
    let new_edge = occt::interpolated_edge(&points, &derivatives, &parameters, false).map_err(occt_error)?;
    let from = occt::connect_edges(std::slice::from_ref(&other)).map_err(occt_error)?;
    let to = occt::connect_edges(std::slice::from_ref(&new_edge)).map_err(occt_error)?;
    let (Some(from), Some(to)) = (from.first(), to.first()) else { return Err(failed()) };
    let sheet = occt::loft(&[from.clone(), to.clone()], false).map_err(|_| failed())?.shape();
    // The old corners stay as vertices on the straight sides.
    Ok(Some((sheet, vec![e0, e1])))
}

/// A ruled sheet of one face extended past one of its straight sides (a ruling): each curve across it carried on
/// straight, along the surface, by the distance (batch 066 c002: Onshape's new edge is 3 mm from the old corners
/// along both arcs). One face, lofted through those curves; its curved edges keep vertices at the old corners.
fn extend_ruled_along(face: &Shape, own: &[Shape], side: &Shape, distance: f64) -> R<Option<(Shape, Vec<occt::Vec3>)>> {
    use crate::values::{add, norm, scale, sub};
    let failed = || regen_error("EXTEND_SHEET_BODY_FAILED");
    let ends = |e: &Shape| -> R<(occt::Vec3, occt::Vec3)> {
        Ok((e.edge_tangent(0.0, false).map_err(occt_error)?.origin, e.edge_tangent(1.0, false).map_err(occt_error)?.origin))
    };
    let close = |a: occt::Vec3, b: occt::Vec3| norm(sub(a, b)) < 1e-7;
    let (s0, s1) = ends(side)?;
    // The face's parameter direction along the curves: the side lies at one end of it.
    let (f0u, f0v) = face.face_fraction(s0).map_err(occt_error)?;
    let (f1u, f1v) = face.face_fraction(s1).map_err(occt_error)?;
    let side_is_u_end = (f0u - f1u).abs() < 1e-6 && (f0u < 1e-6 || f0u > 1.0 - 1e-6);
    let side_is_v_end = (f0v - f1v).abs() < 1e-6 && (f0v < 1e-6 || f0v > 1.0 - 1e-6);
    let (along_u, at_end) = match (side_is_u_end, side_is_v_end) {
        (true, false) => (true, f0u > 0.5),
        (false, true) => (false, f0v > 0.5),
        _ => return Ok(None),
    };
    let point = |a: f64, c: f64| -> R<occt::Frame> {
        let (u, v) = if along_u { (a, c) } else { (c, a) };
        face.face_tangent_plane(u, v).map_err(occt_error)
    };
    const SECTIONS: usize = 9;
    const SAMPLES: usize = 32;
    let mut wires = Vec::new();
    for k in 0..=SECTIONS {
        let c = k as f64 / SECTIONS as f64;
        // The curve across from the far side to this one, then on straight.
        let mut points: Vec<occt::Vec3> = (0..=SAMPLES)
            .map(|i| {
                let a = i as f64 / SAMPLES as f64;
                point(if at_end { a } else { 1.0 - a }, c).map(|f| f.origin)
            })
            .collect::<R<_>>()?;
        let n = points.len();
        let tip = points[n - 1];
        let h = 1e-6;
        let before = point(if at_end { 1.0 - h } else { h }, c)?.origin;
        let direction = crate::values::normalize(sub(tip, before)).ok_or_else(failed)?;
        for i in 1..=8 {
            points.push(add(tip, scale(direction, distance * i as f64 / 8.0)));
        }
        let mut parameters = vec![0.0];
        for w in points.windows(2) {
            parameters.push(parameters.last().copied().unwrap_or(0.0) + norm(sub(w[1], w[0])));
        }
        let m = points.len();
        let derivatives: Vec<occt::Vec3> = (0..m)
            .map(|j| {
                if j >= n - 1 {
                    return direction;
                }
                let (a, b) = if j == 0 { (0, 1) } else { (j - 1, j + 1) };
                scale(sub(points[b], points[a]), 1.0 / (parameters[b] - parameters[a]))
            })
            .collect();
        let curve = occt::interpolated_edge(&points, &derivatives, &parameters, false).map_err(occt_error)?;
        wires.extend(occt::connect_edges(std::slice::from_ref(&curve)).map_err(occt_error)?);
    }
    let sheet = occt::loft(&wires, false).map_err(|_| failed())?.shape();
    // The old corners stay as vertices on the curved edges.
    let _ = (own, close);
    Ok(Some((sheet, vec![s0, s1])))
}

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
    // An elliptical arc is carried on round its ellipse by the distance as arc length, curving on or not (batch 119
    // c003, c004), as a circular one is round its cylinder.
    let mut ellipse = ellipse_range(bottom)?;
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
            if let Some(range) = ellipse.as_mut() {
                range.extend(!at_start, distance);
            } else {
                profile = extended_cubic(profile, !at_start, curving, distance, true)?;
            }
        } else {
            return Ok(None);
        }
    }
    let curve = match &ellipse {
        Some(range) => range.edge()?,
        None => profile.edge()?,
    };
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

/// An elliptical arc as its ellipse and parameter range (radians from the major axis), its start where the edge
/// starts.
struct EllipseRange {
    frame: occt::Frame,
    major: f64,
    minor: f64,
    start: f64,
    end: f64,
}

impl EllipseRange {
    /// The arc length along the ellipse between two of its parameters.
    fn length(&self, t0: f64, t1: f64) -> f64 {
        let n = 256;
        let h = (t1 - t0) / n as f64;
        let speed = |t: f64| (self.major * self.major * t.sin().powi(2) + self.minor * self.minor * t.cos().powi(2)).sqrt();
        // Simpson's rule.
        let mut sum = speed(t0) + speed(t1);
        for i in 1..n {
            sum += speed(t0 + i as f64 * h) * if i % 2 == 1 { 4.0 } else { 2.0 };
        }
        sum * h / 3.0
    }

    /// The arc carried on by `distance` along the ellipse past its end (`at_end`) or its start.
    fn extend(&mut self, at_end: bool, distance: f64) {
        let way = if (self.end > self.start) == at_end { 1.0 } else { -1.0 };
        let from = if at_end { self.end } else { self.start };
        // Newton's method on the arc length, from the circle of the larger radius.
        let mut t = from + way * distance / self.major;
        for _ in 0..30 {
            let speed = (self.major * self.major * t.sin().powi(2) + self.minor * self.minor * t.cos().powi(2)).sqrt();
            let error = self.length(from.min(t), from.max(t)) - distance;
            t -= way * error / speed;
        }
        if at_end {
            self.end = t;
        } else {
            self.start = t;
        }
    }

    fn edge(&self) -> R<Shape> {
        let (a, b) = if self.start < self.end { (self.start, self.end) } else { (self.end, self.start) };
        occt::ellipse_edge(self.frame, self.major, self.minor, Some((a, b))).map_err(occt_error)
    }
}

/// An edge on an ellipse as an [`EllipseRange`], or `None` for any other edge.
fn ellipse_range(edge: &Shape) -> R<Option<EllipseRange>> {
    use crate::values::{cross, dot, sub};
    if edge.curve_type().ok() != Some(occt::CurveType::Ellipse) {
        return Ok(None);
    }
    let conic = edge.curve_definition().map_err(occt_error)?;
    let frame = conic.frame;
    let y = cross(frame.normal, frame.x);
    let angle = |p: occt::Vec3| {
        let d = sub(p, frame.origin);
        (dot(d, y) / conic.radius2).atan2(dot(d, frame.x) / conic.radius1)
    };
    let p0 = edge.edge_tangent(0.0, false).map_err(occt_error)?;
    let p1 = edge.edge_tangent(1.0, false).map_err(occt_error)?;
    let (start, mut end) = (angle(p0.origin), angle(p1.origin));
    // The way round: the edge's tangent at its start, against the ellipse's own direction there.
    let forward = {
        let t = angle(p0.origin);
        let own = crate::values::add(
            crate::values::scale(frame.x, -conic.radius1 * t.sin()),
            crate::values::scale(y, conic.radius2 * t.cos()),
        );
        dot(own, p0.direction) > 0.0
    };
    if forward && end <= start {
        end += std::f64::consts::TAU;
    } else if !forward && end >= start {
        end -= std::f64::consts::TAU;
    }
    Ok(Some(EllipseRange { frame, major: conic.radius1, minor: conic.radius2, start, end }))
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
        if let Some(frame) = onshape_frame
            && let Some(curves) = torus_curves(&face, frame, group, &creation)?
        {
            // Auto-spaced curves that all fall on the face's edges leave none, which Onshape refuses (batch 191: two
            // across a quarter torus).
            if curves.is_empty() && creation.contains("AUTO_SPACED") {
                return Err(regen_error("ISOPARAMETRIC_CURVE_SELECT_POSITION"));
            }
            let names = field(group, "names").as_array();
            for (i, edge) in curves.into_iter().enumerate() {
                let mut curve_id = id.to_vec();
                if let Some(name) = names.and_then(|n| n.get(i)).and_then(Value::as_str) {
                    curve_id.push(name.into());
                }
                add_wires(model, &curve_id.into(), &[edge])?;
            }
            continue;
        }
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
        // Round a face that wraps (a quarter torus wraps round its tube, not round its axis, batch 073 c004).
        let periodic = occt::face_periodic(&face, !along_u) && {
            let ((u0, v0), (u1, v1)) = face.face_uv_bounds().map_err(occt_error)?;
            let span = if !along_u { u1 - u0 } else { v1 - v0 };
            span >= 2.0 * std::f64::consts::PI - 1e-9
        };
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
        let mut made = 0;
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
            made += edges.len();
            add_wires(model, &curve_id.into(), &edges)?;
        }
        // None left (all on the face's edges, as two across a cylinder's height): Onshape refuses (batch 191 c004).
        if auto && made == 0 {
            return Err(regen_error("ISOPARAMETRIC_CURVE_SELECT_POSITION"));
        }
    }
    Ok(Value::UNDEFINED)
}

/// Isoparametric curves on a torus with Onshape's coordinate system `frame` (batch 085, 073 c004–c005), in Onshape's
/// own parameters: P(u, v) = C + (R − r cos v)(cos u x + sin u y) − r sin v z, v from the tube's inner equator. DIR1
/// curves are tube circles at fixed u, starting at the face's least v and running round it; DIR2 ones run round the
/// axis at fixed v from its least u. Equally spaced round a whole period at k / n from zero, across a part of one at
/// k / (n − 1) without the ends (which are the face's edges). None for other faces.
fn torus_curves(face: &Shape, frame: occt::Frame, group: &Value, creation: &str) -> R<Option<Vec<Shape>>> {
    use crate::values::{add, cross, scale};
    use std::f64::consts::TAU;
    let Ok(s) = face.surface_definition() else { return Ok(None) };
    if s.kind != 4 {
        return Ok(None);
    }
    let Some(fractions) = super::frames::torus_fractions(face, frame) else { return Ok(None) };
    let (big, small) = (s.radius1, s.radius2);
    let (c, z, x) = (frame.origin, frame.normal, frame.x);
    let y = cross(z, x);
    // Onshape's ranges: where its fractions 0 and 1 fall.
    let ((u0, v0), (u1, v1)) = face.face_uv_bounds().map_err(occt_error)?;
    let (su, sv) = (u1 - u0, v1 - v0);
    let at = |fu: f64, fv: f64| -> R<occt::Vec3> {
        let (a, b) = fractions(fu, fv);
        Ok(face.face_tangent_plane(a, b).map_err(occt_error)?.origin)
    };
    // The start of Onshape's ranges, from the point at fractions (0, 0).
    let start = at(0.0, 0.0)?;
    let rel = crate::values::sub(start, c);
    let height = crate::values::dot(rel, z);
    let flat = crate::values::sub(rel, scale(z, height));
    let lo_u = crate::values::dot(flat, y).atan2(crate::values::dot(flat, x)).rem_euclid(TAU);
    let rho = crate::values::norm(flat);
    let lo_v = (-height).atan2(big - rho).rem_euclid(TAU);
    // Onshape's tube angle at a point on the torus.
    let tube_angle = |p: occt::Vec3| {
        let rel = crate::values::sub(p, c);
        let height = crate::values::dot(rel, z);
        let rho = crate::values::norm(crate::values::sub(rel, scale(z, height)));
        (-height).atan2(big - rho).rem_euclid(TAU)
    };
    // A whole torus's seam: the revolved profile circle, which in Onshape starts a quarter turn back from its x
    // (a sketch circle's -90°), where the engine's starts at its x (batch 085 c002, c004).
    let seam_v = face.sub_shapes(ShapeKind::Edge).iter().find_map(|e| {
        let conic = e.curve_definition().ok()?;
        let f = conic.frame;
        let in_axis_plane = crate::values::dot(f.normal, z).abs() < 1e-9;
        (conic.kind == 1 && (conic.radius1 - small).abs() < 1e-9 && in_axis_plane)
            .then(|| tube_angle(crate::values::sub(f.origin, scale(cross(f.normal, f.x), small))))
    });
    let dir1 = creation.starts_with("DIR1");
    let span = if dir1 { su } else { sv };
    let whole = |s: f64| s >= TAU - 1e-9;
    let fractions_wanted: Vec<f64> = if creation.contains("AUTO_SPACED") {
        let n = field(group, "nCurves").as_number().unwrap_or(1.0).max(1.0) as usize;
        if whole(span) {
            (0..n).map(|i| i as f64 / n as f64).collect()
        } else if n == 1 {
            vec![0.5]
        } else {
            // At i / (n − 1) without the ends: two curves leave none (batch 191).
            (1..n - 1).map(|i| i as f64 / (n - 1) as f64).collect()
        }
    } else {
        values::numbers(field(group, "parameters"), "parameters")?
    };
    let mut curves = Vec::new();
    for f in fractions_wanted {
        let edge = if dir1 {
            let u = lo_u + f * su;
            let radial = add(scale(x, u.cos()), scale(y, u.sin()));
            let centre = add(c, scale(radial, big));
            let circle = occt::Frame { origin: centre, normal: cross(radial, z), x: scale(radial, -1.0) };
            if let Some(seam_v) = seam_v.filter(|_| whole(sv) && whole(su) && f == 0.0) {
                // The curve on the seam starts where the profile does (batch 085 c002).
                let x = add(scale(radial, -seam_v.cos()), scale(z, -seam_v.sin()));
                occt::circle_edge(occt::Frame { x, ..circle }, small)
            } else if whole(sv) {
                occt::circle_edge(circle, small)
            } else {
                occt::arc_edge(circle, small, lo_v, lo_v + sv)
            }
        } else {
            let v = lo_v + f * sv;
            let circle = occt::Frame { origin: add(c, scale(z, -small * v.sin())), normal: z, x };
            let radius = big - small * v.cos();
            if whole(su) {
                occt::circle_edge(occt::Frame { x: add(scale(x, lo_u.cos()), scale(y, lo_u.sin())), ..circle }, radius)
            } else {
                occt::arc_edge(circle, radius, lo_u, lo_u + su)
            }
        }
        .map_err(occt_error)?;
        curves.push(edge);
    }
    Ok(Some(curves))
}

pub fn boundary_surface(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let profiles = |key: &str| -> R<Vec<Shape>> {
        let mut wires = Vec::new();
        for q in field(definition, key).as_array().map(|a| a.to_vec()).unwrap_or_default() {
            let edges = shapes(model, &q, EntityType::Edge)?;
            let mut connected = occt::connect_edges(&edges).map_err(occt_error)?;
            if connected.len() != 1 {
                return Err(regen_error("BSURF_FAILED"));
            }
            wires.push(connected.remove(0));
        }
        Ok(wires)
    };
    let (u, v) = (profiles("uProfileSubqueries")?, profiles("vProfileSubqueries")?);
    // Profiles one way only make the surface ruled between them; v profiles along the u profiles' ruled surface's
    // sides change nothing (batch 062 c002: the same poles as without them).
    let both = !u.is_empty() && !v.is_empty();
    let conditions = |key: &str| field(definition, key).as_array().map(|a| a.to_vec()).unwrap_or_default();
    let (u_conditions, v_conditions) = (conditions("uDerivativeInfo"), conditions("vDerivativeInfo"));
    // Profiles one way only, given as v profiles: taken as u profiles, their end conditions with them, as without
    // conditions (batch 159 c013, c014).
    let (u, v, u_conditions, v_conditions) =
        if u.is_empty() { (v, u, v_conditions, u_conditions) } else { (u, v, u_conditions, v_conditions) };
    if !u_conditions.is_empty() || !v_conditions.is_empty() {
        if !v_conditions.is_empty() || u_conditions.len() > 2 {
            return Err(error("@opBoundarySurface: end conditions on v profiles aren't supported yet"));
        }
        if both || u.len() != 2 {
            return Err(error("@opBoundarySurface: end conditions with more than two profiles aren't supported yet"));
        }
        let mut ends: [Option<EndCondition>; 2] = [None, None];
        for info in &u_conditions {
            let index = (field(info, "profileIndex").as_number().unwrap_or(0.0) as usize).min(1);
            let magnitude = field(info, "magnitude").as_number().unwrap_or(1.0);
            // Matching a flat face next to the profile: tangent to the face's plane, as tangent to a profile's own
            // plane is (batch 159).
            if !field(info, "adjacentFaces").is_undefined() {
                if values::opt_bool(info, "matchCurvature")?.unwrap_or(false) {
                    return Err(error("@opBoundarySurface: matching adjacent faces' curvature isn't supported yet"));
                }
                // (A flat face of another kind, a sheet extruded from a straight spline, as its plane: batch 163.)
                let flat_normal = |f: &Shape| -> Option<occt::Vec3> {
                    if let Ok(p) = f.face_plane() {
                        return Some(p.normal);
                    }
                    let grid: Vec<occt::Frame> = [0.0, 0.5, 1.0]
                        .iter()
                        .flat_map(|&u| [0.0, 0.5, 1.0].map(|v| f.face_tangent_plane(u, v).ok()))
                        .collect::<Option<_>>()?;
                    let n = grid[0].normal;
                    grid.iter()
                        .all(|t| {
                            values::norm(values::cross(t.normal, n)) < 1e-9
                                && values::dot(values::sub(t.origin, grid[0].origin), n).abs() < 1e-7
                        })
                        .then_some(n)
                };
                let adjacent = shapes(model, field(info, "adjacentFaces"), EntityType::Face)?;
                let normals: Vec<occt::Vec3> = adjacent
                    .iter()
                    .map(flat_normal)
                    .collect::<Option<_>>()
                    .ok_or_else(|| error("@opBoundarySurface: matching curved adjacent faces isn't supported yet"))?;
                let Some(&normal) = normals.first() else { return Err(regen_error("BSURF_FAILED")) };
                // Faces in several planes, each edge leaving tangent to its own (batch 166 c003).
                let faces = adjacent.into_iter().zip(normals).collect();
                ends[index] = Some(EndCondition { magnitude, tangent: true, vector: normal, faces });
                continue;
            }
            ends[index] = Some(EndCondition {
                magnitude,
                tangent: values::opt_bool(info, "tangentToPlane")?.unwrap_or(false),
                vector: values::vec3(field(info, "vector"), "@opBoundarySurface derivative vector")?,
                faces: Vec::new(),
            });
        }
        let ruled = occt::loft(&u, false).map_err(|_| regen_error("BSURF_FAILED"))?.shape();
        let face = conditioned_surface(&u, &ends, &ruled)?;
        add(model, &id, face, BodyType::Sheet);
        return Ok(Value::UNDEFINED);
    }
    let wires = if u.is_empty() { &v } else { &u };
    let op = occt::loft(wires, false).map_err(|_| regen_error("BSURF_FAILED"))?;
    if both {
        let sides = op.shape();
        for wire in &v {
            let on_side = wire.sub_shapes(ShapeKind::Edge).iter().all(|e| {
                [0.0, 0.25, 0.5, 0.75, 1.0].iter().all(|&t| {
                    e.edge_tangent(t, false)
                        .ok()
                        .and_then(|a| occt::vertex(a.origin).ok())
                        .and_then(|p| p.distance(&sides).ok())
                        .is_some_and(|d| d.distance < 1e-7)
                })
            });
            if !on_side {
                // Two profiles each way meeting at their ends: the Coons patch of the four (batch 063 c009).
                if u.len() == 2 && v.len() == 2 {
                    let face = coons_surface(&u, &v, &sides)?;
                    add(model, &id, face, BodyType::Sheet);
                    return Ok(Value::UNDEFINED);
                }
                return Err(error("@opBoundarySurface: v profiles off the u profiles' ruled surface aren't supported yet"));
            }
        }
    }
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

/// Curves through 33 points each at even fractions of 0 to 1 (`rows(f)` gives each curve's point at `f`), as
/// non-rational cubic B-splines on one knot vector: each span the cubic Hermite segment its ends' points and
/// derivatives (by central differences) fix, the interior knots doubled (C1).
pub(crate) fn sampled_rows(count: usize, rows: impl Fn(f64) -> R<Vec<occt::Vec3>>) -> R<Vec<occt::BSplineData>> {
    sampled_rows_in(count, 33, rows)
}

/// [`sampled_rows`] through `stations` points each (`stations - 1` spans).
pub(crate) fn sampled_rows_in(
    count: usize,
    stations: usize,
    rows: impl Fn(f64) -> R<Vec<occt::Vec3>>,
) -> R<Vec<occt::BSplineData>> {
    let params: Vec<f64> = (0..stations).map(|i| i as f64 / (stations - 1) as f64).collect();
    sampled_rows_at(count, &params, rows)
}

/// [`sampled_rows`] through points at the given fractions (increasing, from 0 to 1), the knots there.
pub(crate) fn sampled_rows_at(
    count: usize,
    params: &[f64],
    rows: impl Fn(f64) -> R<Vec<occt::Vec3>>,
) -> R<Vec<occt::BSplineData>> {
    use crate::values::{add, scale, sub};
    let stations = params.len();
    let samples: Vec<Vec<occt::Vec3>> = params.iter().map(|&f| rows(f)).collect::<R<_>>()?;
    if samples.iter().any(|s| s.len() != count) {
        return Err(error("sampled rows came out uneven"));
    }
    let h = 1e-5;
    let slopes: Vec<Vec<occt::Vec3>> = params
        .iter()
        .map(|&f| -> R<Vec<occt::Vec3>> {
            let (lo, hi) = ((f - h).max(0.0), (f + h).min(1.0));
            let (a, b) = (rows(lo)?, rows(hi)?);
            Ok(a.iter().zip(&b).map(|(&p, &q)| scale(sub(q, p), 1.0 / (hi - lo))).collect())
        })
        .collect::<R<_>>()?;
    let mut knots = vec![0.0; 4];
    for &f in &params[1..stations - 1] {
        knots.extend([f, f]);
    }
    knots.extend([1.0; 4]);
    Ok((0..count)
        .map(|row| {
            let mut poles = vec![samples[0][row]];
            for k in 0..stations - 1 {
                let span = params[k + 1] - params[k];
                poles.push(add(samples[k][row], scale(slopes[k][row], span / 3.0)));
                poles.push(sub(samples[k + 1][row], scale(slopes[k + 1][row], span / 3.0)));
            }
            poles.push(samples[stations - 1][row]);
            occt::BSplineData { degree: 3, periodic: false, rational: false, poles, weights: Vec::new(), knots: knots.clone() }
        })
        .collect())
}

/// A curve's non-rational cubic B-spline through points at even fractions of its parameter, from its start if
/// `reversed` is false, else from its end.
fn sampled_curve(edge: &Shape, reversed: bool) -> R<occt::BSplineData> {
    let mut curves = sampled_rows(1, |f| {
        Ok(vec![edge.edge_tangent(if reversed { 1.0 - f } else { f }, false).map_err(occt_error)?.origin])
    })?;
    Ok(curves.remove(0))
}

/// The Greville abscissae of a B-spline's poles: where the linear function's poles lie.
fn greville(curve: &occt::BSplineData) -> Vec<f64> {
    let k = curve.degree as usize;
    let (lo, hi) = (curve.knots[0], curve.knots[curve.knots.len() - 1]);
    (0..curve.poles.len()).map(|i| (curve.knots[i + 1..=i + k].iter().sum::<f64>() / k as f64 - lo) / (hi - lo)).collect()
}

/// A boundary surface through two u profiles and two v profiles meeting at their ends: the bilinearly blended
/// Coons patch of the four, as Onshape's is (batch 063 c009: to 7e-7 mm, from its own fits of the arcs). Each pair
/// of opposite curves is sampled alike, and the patch's poles are the two ruled surfaces' less the bilinear one's,
/// each at the Greville abscissae (exact: a linear function's poles lie there). Faces as the u profiles' ruled
/// surface `ruled` does.
fn coons_surface(u: &[Shape], v: &[Shape], ruled: &Shape) -> R<Shape> {
    use crate::values::{add, dot, norm, scale, sub};
    let single = |w: &Shape| -> R<Shape> {
        match &w.sub_shapes(ShapeKind::Edge)[..] {
            [e] => Ok(e.clone()),
            _ => Err(error("@opBoundarySurface: profiles of more than one edge across each other aren't supported yet")),
        }
    };
    let end = |e: &Shape, f: f64| -> R<occt::Vec3> { Ok(e.edge_tangent(f, false).map_err(occt_error)?.origin) };
    let near = |a: occt::Vec3, b: occt::Vec3| norm(sub(a, b)) < 1e-7;
    let (a, b, c, d) = (single(&u[0])?, single(&u[1])?, single(&v[0])?, single(&v[1])?);
    let (a0, a1) = (end(&a, 0.0)?, end(&a, 1.0)?);
    // C runs from A's start, D from A's end; B from C's other end.
    let orient = |e: &Shape, from: occt::Vec3| -> R<Option<bool>> {
        Ok(if near(end(e, 0.0)?, from) { Some(false) } else if near(end(e, 1.0)?, from) { Some(true) } else { None })
    };
    let (c, d, c_rev, d_rev) = match (orient(&c, a0)?, orient(&d, a1)?) {
        (Some(cr), Some(dr)) => (c, d, cr, dr),
        _ => match (orient(&d, a0)?, orient(&c, a1)?) {
            (Some(cr), Some(dr)) => (d, c, cr, dr),
            _ => return Err(regen_error("BSURF_FAILED")),
        },
    };
    let c1 = end(&c, if c_rev { 0.0 } else { 1.0 })?;
    let d1 = end(&d, if d_rev { 0.0 } else { 1.0 })?;
    let b_rev = match orient(&b, c1)? {
        Some(r) => r,
        None => return Err(regen_error("BSURF_FAILED")),
    };
    if !near(end(&b, if b_rev { 0.0 } else { 1.0 })?, d1) {
        return Err(regen_error("BSURF_FAILED"));
    }
    let (ca, cb, cc, cd) = (sampled_curve(&a, false)?, sampled_curve(&b, b_rev)?, sampled_curve(&c, c_rev)?, sampled_curve(&d, d_rev)?);
    if ca.knots != cb.knots || cc.knots != cd.knots || ca.poles.len() != cb.poles.len() || cc.poles.len() != cd.poles.len() {
        return Err(regen_error("BSURF_FAILED"));
    }
    let (tau, sigma) = (greville(&ca), greville(&cc));
    let corners = [ca.poles[0], ca.poles[ca.poles.len() - 1], cb.poles[0], cb.poles[cb.poles.len() - 1]];
    let (nu, nv) = (ca.poles.len(), cc.poles.len());
    let mut poles = Vec::with_capacity(nu * nv);
    for i in 0..nu {
        for j in 0..nv {
            let (t, s) = (tau[i], sigma[j]);
            let ruled_u = add(scale(ca.poles[i], 1.0 - s), scale(cb.poles[i], s));
            let ruled_v = add(scale(cc.poles[j], 1.0 - t), scale(cd.poles[j], t));
            let bilinear = add(
                add(scale(corners[0], (1.0 - t) * (1.0 - s)), scale(corners[1], t * (1.0 - s))),
                add(scale(corners[2], (1.0 - t) * s), scale(corners[3], t * s)),
            );
            poles.push(sub(add(ruled_u, ruled_v), bilinear));
        }
    }
    let face = occt::bspline_surface_face(&poles, (nu, nv), &ca.knots, &cc.knots, (ca.degree, cc.degree))
        .map_err(|_| regen_error("BSURF_FAILED"))?;
    let facing = |s: &Shape| s.sub_shapes(ShapeKind::Face).first().and_then(|f| f.face_tangent_plane(0.5, 0.5).ok()).map(|t| t.normal);
    Ok(match (facing(&face), facing(ruled)) {
        (Some(x), Some(y)) if dot(x, y) < 0.0 => face.reversed(),
        _ => face,
    })
}

/// A boundary surface's end condition at a profile: a derivative along `vector` (normal to the profile's plane, or a
/// direction given) or, `tangent`, in the plane square to it.
#[derive(Clone)]
struct EndCondition {
    magnitude: f64,
    tangent: bool,
    vector: occt::Vec3,
    /// Matching flat faces next to the profile: each face with its plane's normal (empty otherwise).
    faces: Vec<(Shape, occt::Vec3)>,
}

/// A profile edge as Onshape takes it into a boundary surface: a cubic B-spline over 0 to 1, running as the edge does
/// (batch 161, 163): a line raised to a cubic; an arc as a cubic of even spans over even angles, a power of two of
/// them, none over 22.5° (four for 87°, eight for 135°, batch 063), its ends and end derivatives exact and the rest a
/// least-squares fit (to 1e-5 of Onshape's poles); a non-rational cubic spline as it is. Others: `None`.
fn profile_spline(edge: &Shape) -> Option<(Vec<occt::Vec3>, Vec<f64>)> {
    use crate::values::{add, norm, scale, sub};
    let at = |f: f64| edge.edge_tangent(f, false).ok();
    let (start, end) = (at(0.0)?, at(1.0)?);
    match edge.curve_type().ok()? {
        occt::CurveType::Line => {
            let (a, b) = (start.origin, end.origin);
            let d = sub(b, a);
            Some(((0..4).map(|k| add(a, scale(d, k as f64 / 3.0))).collect(), vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]))
        }
        occt::CurveType::Circle => {
            let centre = edge.curve_definition().ok()?.frame.origin;
            let (x, y, radius, angle) =
                crate::curve_extend::arc_frame(centre, start.origin, start.direction, end.origin).ok()?;
            let mut spans = 1usize;
            while angle / spans as f64 > 22.5f64.to_radians() + 1e-9 {
                spans *= 2;
            }
            let point = |f: f64| add(centre, add(scale(x, radius * (f * angle).cos()), scale(y, radius * (f * angle).sin())));
            let speed = |f: f64| add(scale(x, -radius * angle * (f * angle).sin()), scale(y, radius * angle * (f * angle).cos()));
            let mut knots = vec![0.0; 4];
            knots.extend((1..spans).map(|k| k as f64 / spans as f64));
            knots.extend([1.0; 4]);
            let n = spans + 3;
            let h = 1.0 / spans as f64;
            let mut poles = vec![occt::Vec3::new(0.0, 0.0, 0.0); n];
            poles[0] = point(0.0);
            poles[1] = add(point(0.0), scale(speed(0.0), h / 3.0));
            poles[n - 2] = sub(point(1.0), scale(speed(1.0), h / 3.0));
            poles[n - 1] = point(1.0);
            let free: Vec<usize> = (2..n - 2).collect();
            if !free.is_empty() {
                let samples: Vec<f64> = (0..=64 * spans).map(|k| k as f64 / (64 * spans) as f64).collect();
                let fixed = [0, 1, n - 2, n - 1];
                let rows: Vec<(Vec<f64>, occt::Vec3)> = samples
                    .iter()
                    .map(|&f| {
                        let b = basis_at(&knots, n, f);
                        let rest = fixed.iter().fold(occt::Vec3::new(0.0, 0.0, 0.0), |acc, &i| add(acc, scale(poles[i], b[i])));
                        (free.iter().map(|&i| b[i]).collect(), sub(point(f), rest))
                    })
                    .collect();
                let solved = least_squares(&rows, &[], 0.0)?;
                for (k, &i) in free.iter().enumerate() {
                    poles[i] = solved[k];
                }
            }
            Some((poles, knots))
        }
        occt::CurveType::BSpline => {
            let data = edge.edge_bspline(1e-7, false, false).ok()?;
            if data.degree != 3 || data.rational || data.periodic {
                return None;
            }
            let (k0, k1) = (*data.knots.first()?, *data.knots.last()?);
            let mut knots: Vec<f64> = data.knots.iter().map(|k| (k - k0) / (k1 - k0)).collect();
            let mut poles = data.poles.clone();
            if norm(sub(poles[0], start.origin)) > norm(sub(poles[0], end.origin)) {
                poles.reverse();
                knots = knots.iter().rev().map(|k| 1.0 - k).collect();
            }
            Some((poles, knots))
        }
        _ => None,
    }
}

/// The cubic B-spline basis functions at `u`, one per pole.
fn basis_at(knots: &[f64], n: usize, u: f64) -> Vec<f64> {
    let span = crate::bspline::find_span(n, 3, u, knots);
    let b = crate::bspline::basis_derivatives(span, u, 3, 0, knots);
    let mut out = vec![0.0; n];
    for j in 0..4 {
        out[span - 3 + j] = b[0][j];
    }
    out
}

/// The least-squares solution of rows `(coefficients, target)`, plus `pull` times the distance of each unknown from
/// `anchors` (if given), by the normal equations.
fn least_squares(rows: &[(Vec<f64>, occt::Vec3)], anchors: &[occt::Vec3], pull: f64) -> Option<Vec<occt::Vec3>> {
    let k = rows.first().map(|r| r.0.len())?;
    let mut a = vec![vec![0.0; k]; k];
    let mut b = vec![[0.0; 3]; k];
    for (c, t) in rows {
        for i in 0..k {
            for j in 0..k {
                a[i][j] += c[i] * c[j];
            }
            b[i][0] += c[i] * t.x;
            b[i][1] += c[i] * t.y;
            b[i][2] += c[i] * t.z;
        }
    }
    for i in 0..k {
        if let Some(anchor) = anchors.get(i) {
            a[i][i] += pull;
            b[i][0] += pull * anchor.x;
            b[i][1] += pull * anchor.y;
            b[i][2] += pull * anchor.z;
        }
    }
    // Gaussian elimination with partial pivoting.
    for col in 0..k {
        let pivot = (col..k).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[pivot][col].abs() < 1e-300 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for row in col + 1..k {
            let f = a[row][col] / a[col][col];
            for c2 in col..k {
                a[row][c2] -= f * a[col][c2];
            }
            for d in 0..3 {
                b[row][d] -= f * b[col][d];
            }
        }
    }
    let mut x = vec![[0.0; 3]; k];
    for row in (0..k).rev() {
        for d in 0..3 {
            let s: f64 = (row + 1..k).map(|c2| a[row][c2] * x[c2][d]).sum();
            x[row][d] = (b[row][d] - s) / a[row][row];
        }
    }
    Some(x.into_iter().map(|v| occt::Vec3::new(v[0], v[1], v[2])).collect())
}

/// A cubic B-spline with the knot `u` inserted once (Boehm).
fn insert_knot(poles: &[occt::Vec3], knots: &[f64], u: f64) -> (Vec<occt::Vec3>, Vec<f64>) {
    use crate::values::{add, scale};
    let n = poles.len();
    let k = crate::bspline::find_span(n, 3, u, knots);
    let mut out = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let q = if i + 3 <= k {
            poles[i]
        } else if i > k {
            poles[i - 1]
        } else {
            let alpha = (u - knots[i]) / (knots[i + 3] - knots[i]);
            add(scale(poles[i], alpha), scale(poles[i - 1], 1.0 - alpha))
        };
        out.push(q);
    }
    let mut new_knots = knots.to_vec();
    new_knots.insert(k + 1, u);
    (out, new_knots)
}

/// Two cubic B-splines over 0 to 1 on one knot vector: each given the other's knots it lacks.
fn compatible(
    a: (Vec<occt::Vec3>, Vec<f64>),
    b: (Vec<occt::Vec3>, Vec<f64>),
) -> ((Vec<occt::Vec3>, Vec<f64>), (Vec<occt::Vec3>, Vec<f64>)) {
    let interior = |k: &[f64]| -> Vec<f64> { k[4..k.len() - 4].to_vec() };
    let missing = |have: &[f64], want: &[f64]| -> Vec<f64> {
        let mut have = have.to_vec();
        let mut out = Vec::new();
        for &w in want {
            if let Some(i) = have.iter().position(|h| (h - w).abs() < 1e-9) {
                have.remove(i);
            } else {
                out.push(w);
            }
        }
        out
    };
    let (ia, ib) = (interior(&a.1), interior(&b.1));
    let (mut a, mut b) = (a, b);
    for u in missing(&ia, &ib) {
        a = insert_knot(&a.0, &a.1, u);
    }
    for u in missing(&ib, &ia) {
        b = insert_knot(&b.0, &b.1, u);
    }
    (a, b)
}

/// A boundary surface between two profiles, conditioned tangent to planes, built as Onshape builds it, pole by pole
/// on the knots across its profiles (batches 161–163): each conditioned profile leaves along its chord to the other,
/// the part along the plane's normal taken out, pole by pole; at a magnitude other than 1 the leaving derivative's
/// interior poles are a least-squares fit, at their own places along the profiles (Greville abscissae), to the
/// magnitude times the magnitude-1 derivative, pulled toward the magnitude-1 poles with weight 0.04, the first two and
/// last two the magnitude times them (to 1e-6 of Onshape's, batches 162, 163); a free profile is reached as the quadratic's, raised to a cubic, plus at each pole the correction
/// blended from the profiles' ends (batch 159 c005). `None` where it doesn't apply (other conditions or curves).
fn poled_surface(profiles: &[Shape], ends: &[Option<EndCondition>; 2], ruled: &Shape) -> R<Option<Shape>> {
    use crate::values::{add, cross, dot, norm, normalize, scale, sub};
    if profiles.len() != 2 || ends.iter().all(Option::is_none) {
        return Ok(None);
    }
    // Normal to a curved profile's plane at a magnitude other than 1, built the same way, the chord's part across
    // the profile within its plane taken out (batch 117 c002, c006); at magnitude 1 the sampled construction
    // ([`conditioned_surface`]) comes nearer Onshape's.
    let curved = profiles.iter().any(|w| w.sub_shapes(ShapeKind::Edge).iter().any(|e| e.curve_type().ok() != Some(occt::CurveType::Line)));
    let normal_ends = ends.iter().flatten().any(|e| !e.tangent);
    let other_magnitude = ends.iter().flatten().any(|e| (e.magnitude - 1.0).abs() > 1e-12);
    if normal_ends && !(curved && other_magnitude && ends.iter().flatten().all(|e| e.faces.is_empty())) {
        return Ok(None);
    }
    // A profile of several edges in a row along one line is that line (batch 166 c001, c004).
    let joined = |w: &Shape| -> Option<(Vec<occt::Vec3>, Vec<f64>)> {
        let edges = w.sub_shapes(ShapeKind::Edge);
        match &edges[..] {
            [e] => profile_spline(e),
            [] => None,
            _ => {
                if edges.iter().any(|e| e.curve_type().ok() != Some(occt::CurveType::Line)) {
                    return None;
                }
                let points: Vec<occt::Vec3> = edges
                    .iter()
                    .flat_map(|e| [0.0, 1.0].map(|t| e.edge_tangent(t, false).ok().map(|l| l.origin)))
                    .collect::<Option<_>>()?;
                let d = normalize(sub(points[1], points[0]))?;
                let along = |p: occt::Vec3| dot(sub(p, points[0]), d);
                if points.iter().any(|p| norm(sub(sub(*p, points[0]), scale(d, along(*p)))) > 1e-7) {
                    return None;
                }
                let lo = points.iter().copied().min_by(|x, y| along(*x).total_cmp(&along(*y)))?;
                let hi = points.iter().copied().max_by(|x, y| along(*x).total_cmp(&along(*y)))?;
                let span = sub(hi, lo);
                Some(((0..4).map(|k| add(lo, scale(span, k as f64 / 3.0))).collect(), vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]))
            }
        }
    };
    let (Some(a), Some(b)) = (joined(&profiles[0]), joined(&profiles[1])) else { return Ok(None) };
    // The partner runs the same way as the first profile.
    let (a0, a1) = (a.0[0], *a.0.last().expect("poles"));
    let (b0, b1) = (b.0[0], *b.0.last().expect("poles"));
    let b = if norm(sub(a0, b0)) + norm(sub(a1, b1)) > norm(sub(a0, b1)) + norm(sub(a1, b0)) {
        (b.0.iter().rev().copied().collect(), b.1.iter().rev().map(|k| 1.0 - k).collect())
    } else {
        b
    };
    // Each conditioned profile must lie square to its planes' normals; where its edges lie along faces in different
    // planes, a knot where they change (batch 166 c003).
    let mut normals = [None, None];
    let (mut a, mut b) = (a, b);
    for (index, end) in ends.iter().enumerate() {
        let Some(end) = end else { continue };
        let Some(n) = normalize(end.vector) else { return Ok(None) };
        let spline = if index == 0 { &mut a } else { &mut b };
        let planes: Vec<occt::Vec3> = if end.faces.is_empty() { vec![n] } else { end.faces.iter().map(|f| f.1).collect() };
        if planes.iter().any(|m| spline.0.iter().any(|p| dot(sub(*p, spline.0[0]), *m).abs() > 1e-7)) {
            return Ok(None);
        }
        // Where along the profile (0 to 1) each face's stretch begins, if the planes change.
        let edges = profiles[index].sub_shapes(ShapeKind::Edge);
        let (start, finish) = (spline.0[0], *spline.0.last().expect("poles"));
        let length = norm(sub(finish, start));
        let fraction = |p: occt::Vec3| if length > 0.0 { dot(sub(p, start), sub(finish, start)) / (length * length) } else { 0.0 };
        let mut stretches: Vec<(f64, f64, occt::Vec3)> = Vec::new();
        for e in &edges {
            let ends_at = [0.0, 1.0].map(|t| e.edge_tangent(t, false).map(|l| fraction(l.origin)).unwrap_or(0.0));
            // (The face the edge's middle lies on: the profile's edges are joined copies.)
            let middle = e.edge_tangent(0.5, false).ok().and_then(|l| occt::vertex(l.origin).ok());
            let normal = end
                .faces
                .iter()
                .find(|(f, _)| middle.as_ref().is_some_and(|m| m.distance(f).is_ok_and(|d| d.distance < 1e-7)))
                .map(|f| f.1)
                .unwrap_or(n);
            stretches.push((ends_at[0].min(ends_at[1]), ends_at[0].max(ends_at[1]), normal));
        }
        stretches.sort_by(|x, y| x.0.total_cmp(&y.0));
        for w in stretches.windows(2) {
            if norm(sub(w[0].2, w[1].2)) > 1e-9 && w[1].0 > 1e-9 && w[1].0 < 1.0 - 1e-9 {
                *spline = insert_knot(&spline.0, &spline.1, w[1].0);
            }
        }
        normals[index] = Some((stretches, end.magnitude, end.tangent));
    }
    let ((p, knots), (q, _)) = compatible(a, b);
    let count = p.len();
    let chords: Vec<occt::Vec3> = p.iter().zip(&q).map(|(a, b)| sub(*b, *a)).collect();
    // Each pole's place along the profiles (its Greville abscissa).
    let greville = |i: usize| (knots[i + 1] + knots[i + 2] + knots[i + 3]) / 3.0;
    // The leaving derivative's poles at an end, the magnitude fitted as Onshape fits it.
    let leaving = |index: usize| -> R<Option<Vec<occt::Vec3>>> {
        let Some((stretches, magnitude, tangent)) = &normals[index] else { return Ok(None) };
        let (magnitude, tangent) = (*magnitude, *tangent);
        // The chord less its part along the plane of the stretch holding `u` (the earlier one where two meet).
        let normal_at = |u: f64| stretches.iter().find(|s| u <= s.1 + 1e-12).or(stretches.last()).map(|s| s.2).expect("a stretch");
        let at = |poles: &[occt::Vec3], u: f64| {
            let basis = basis_at(&knots, count, u);
            poles.iter().zip(&basis).fold(occt::Vec3::new(0.0, 0.0, 0.0), |acc, (p, w)| add(acc, scale(*p, *w)))
        };
        let profile = if index == 0 { &p } else { &q };
        let exact = |u: f64, side: f64| {
            let c = at(&chords, u);
            let n = normal_at(if side < 0.0 { u } else { (u + 1e-9).min(1.0) });
            // Normal to the plane: the part across the profile within it.
            let across = if tangent {
                n
            } else {
                let h = 1e-7;
                let (lo, hi) = ((u - h).max(0.0), (u + h).min(1.0));
                normalize(cross(n, sub(at(profile, hi), at(profile, lo)))).unwrap_or(n)
            };
            sub(c, scale(across, dot(c, across)))
        };
        // Its poles: the first two and last two from its value and slope at the ends, the rest through its values
        // at their places (on one plane, the chords' poles less their parts along the normal).
        let mut plain = vec![occt::Vec3::new(0.0, 0.0, 0.0); count];
        let h = 1e-6;
        let (k1, kn) = (knots[4], knots[knots.len() - 5]);
        plain[0] = exact(0.0, 1.0);
        plain[1] = add(plain[0], scale(sub(exact(h, 1.0), exact(0.0, 1.0)), k1 / (3.0 * h)));
        plain[count - 1] = exact(1.0, -1.0);
        plain[count - 2] = sub(plain[count - 1], scale(sub(exact(1.0, -1.0), exact(1.0 - h, -1.0)), (1.0 - kn) / (3.0 * h)));
        let inner: Vec<usize> = (2..count.saturating_sub(2)).collect();
        if !inner.is_empty() {
            let fixed = [0, 1, count - 2, count - 1];
            let rows: Vec<(Vec<f64>, occt::Vec3)> = inner
                .iter()
                .map(|&i| {
                    let u = greville(i);
                    let basis = basis_at(&knots, count, u);
                    let rest = fixed.iter().fold(occt::Vec3::new(0.0, 0.0, 0.0), |acc, &j| add(acc, scale(plain[j], basis[j])));
                    (inner.iter().map(|&j| basis[j]).collect(), sub(exact(u, -1.0), rest))
                })
                .collect();
            let solved = least_squares(&rows, &[], 0.0).ok_or_else(|| regen_error("BSURF_FAILED"))?;
            for (k, &i) in inner.iter().enumerate() {
                plain[i] = solved[k];
            }
        }
        if plain.iter().zip(&chords).any(|(d, c)| norm(*d) <= 1e-6 * norm(*c).max(1e-9)) {
            return Err(regen_error("CANNOT_MAKE_BSPLINESURFACE"));
        }
        let mut out: Vec<occt::Vec3> = plain.iter().map(|d| scale(*d, magnitude)).collect();
        let free: Vec<usize> = (2..count.saturating_sub(2)).collect();
        if (magnitude - 1.0).abs() > 1e-12 && !free.is_empty() {
            let fixed: Vec<usize> = (0..count).filter(|i| !free.contains(i)).collect();
            // Sampled at the fitted poles' places (on even knots, the interior knots; batch 163 c005, 162 c002,
            // c003 on uneven ones).
            let rows: Vec<(Vec<f64>, occt::Vec3)> = free
                .iter()
                .map(|&i| greville(i))
                .map(|u| {
                    let basis = basis_at(&knots, count, u);
                    let target = (0..count).fold(occt::Vec3::new(0.0, 0.0, 0.0), |acc, i| add(acc, scale(plain[i], basis[i] * magnitude)));
                    let rest = fixed.iter().fold(occt::Vec3::new(0.0, 0.0, 0.0), |acc, &i| add(acc, scale(out[i], basis[i])));
                    (free.iter().map(|&i| basis[i]).collect(), sub(target, rest))
                })
                .collect();
            let anchors: Vec<occt::Vec3> = free.iter().map(|&i| plain[i]).collect();
            let solved = least_squares(&rows, &anchors, 0.04).ok_or_else(|| regen_error("BSURF_FAILED"))?;
            for (k, &i) in free.iter().enumerate() {
                out[i] = solved[k];
            }
        }
        Ok(Some(out))
    };
    let (d0, d1) = (leaving(0)?, leaving(1)?);
    let last = count - 1;
    let mut poles = Vec::with_capacity(count * 4);
    for i in 0..count {
        let (pi, qi, ci) = (p[i], q[i], chords[i]);
        let t = greville(i);
        let row = match (&d0, &d1) {
            (Some(d0), Some(d1)) => [pi, add(pi, scale(d0[i], 1.0 / 3.0)), sub(qi, scale(d1[i], 1.0 / 3.0)), qi],
            (Some(d0), None) => {
                let side = |j: usize| scale(sub(chords[j], d0[j]), 1.0 / 3.0);
                let blend = add(scale(side(0), 1.0 - t), scale(side(last), t));
                let middle = add(add(sub(qi, scale(ci, 2.0 / 3.0)), scale(d0[i], 1.0 / 3.0)), blend);
                [pi, add(pi, scale(d0[i], 1.0 / 3.0)), middle, qi]
            }
            (None, Some(d1)) => {
                let side = |j: usize| scale(sub(d1[j], chords[j]), 1.0 / 3.0);
                let blend = add(scale(side(0), 1.0 - t), scale(side(last), t));
                let middle = add(add(add(pi, scale(ci, 2.0 / 3.0)), scale(d1[i], -1.0 / 3.0)), blend);
                [pi, middle, sub(qi, scale(d1[i], 1.0 / 3.0)), qi]
            }
            (None, None) => return Ok(None),
        };
        poles.extend(row);
    }
    let face = occt::bspline_surface_face(&poles, (count, 4), &knots, &[0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0], (3, 3))
        .map_err(|_| regen_error("BSURF_FAILED"))?;
    // A flat one is a plane, as Onshape's is (batch 121 c001, c002).
    let face = occt::nurbs_converted(&face).map_err(occt_error)?;
    let facing = |s: &Shape| s.sub_shapes(ShapeKind::Face).first().and_then(|f| f.face_tangent_plane(0.5, 0.5).ok()).map(|t| t.normal);
    Ok(Some(match (facing(&face), facing(ruled)) {
        (Some(x), Some(y)) if dot(x, y) < 0.0 => face.reversed(),
        _ => face,
    }))
}

/// A boundary surface between two profiles with end conditions at one or both (batches 063 c010, c011, 117). Across,
/// each curve runs from a profile's point to its partner's (by parameter fraction, as the ruled surface pairs them),
/// leaving a conditioned profile with the chord between them less its part along one direction: normal to the
/// profile's plane, the chord's part across the profile within the plane, keeping the parts along the normal and
/// along the profile; tangent to it, the part along the normal. With one condition the curve is a quadratic, its
/// other end's derivative twice the chord less that (063 c011; 117 c003 at the second profile); with both, the cubic
/// with both (117 c001). So it is in Onshape, within its fits (the derivative's part along the normal is the
/// chord's; along the profile, within 0.01 of it). Where the derivative comes to nothing, as at the ends of two arcs
/// over each other tangent to their planes, Onshape fails (`CANNOT_MAKE_BSPLINESURFACE`, 063 c010); square to a
/// direction the profile doesn't lie square to, it fails as inconsistent (117 c005). Magnitudes other than 1 between
/// curved profiles are built pole by pole ([`poled_surface`]). Along a direction given, the derivative is the chord
/// less its part across the profile within the plane of the direction and the profile's tangent, and a free other
/// end is reached as with conditions normal to a plane (117 c004). The surface is cubic along the profiles through
/// 33 points on each row.
fn conditioned_surface(profiles: &[Shape], ends: &[Option<EndCondition>; 2], ruled: &Shape) -> R<Shape> {
    if let Some(face) = poled_surface(profiles, ends, ruled)? {
        return Ok(face);
    }
    use crate::values::{add, cross, dot, norm, normalize, scale, sub};
    let single = |w: &Shape| -> R<Shape> {
        match &w.sub_shapes(ShapeKind::Edge)[..] {
            [e] => Ok(e.clone()),
            _ => Err(error("@opBoundarySurface: end conditions on profiles of more than one edge aren't supported yet")),
        }
    };
    let (first, second) = (single(&profiles[0])?, single(&profiles[1])?);
    let at = |e: &Shape, f: f64| -> R<occt::Vec3> { Ok(e.edge_tangent(f, false).map_err(occt_error)?.origin) };
    // The partner runs the same way as the first profile.
    let (a0, a1, b0, b1) = (at(&first, 0.0)?, at(&first, 1.0)?, at(&second, 0.0)?, at(&second, 1.0)?);
    let flipped = norm(sub(a0, b0)) + norm(sub(a1, b1)) > norm(sub(a0, b1)) + norm(sub(a1, b0));
    let fraction = |index: usize, f: f64| if index == 1 && flipped { 1.0 - f } else { f };
    let edge = |index: usize| if index == 0 { &first } else { &second };
    // Between two straight profiles every curve across is the cubic leaving a conditioned profile with the magnitude
    // times its projected chord, along a direction given or not, and reaching a free one along the chord itself
    // (batch 121); between curved ones the free end is the quadratic's, and other magnitudes and directions follow
    // rules not found.
    let straight = [&first, &second].iter().all(|e| e.curve_type().ok() == Some(occt::CurveType::Line));
    // Each condition, checked: magnitude 1, and a direction given only square to it (`tangent`), where the profile
    // must lie square to the direction.
    let mut normals = [None, None];
    for (index, end) in ends.iter().enumerate() {
        let Some(end) = end else { continue };
        let n = normalize(end.vector).ok_or_else(|| error("@opBoundarySurface: the derivative vector is zero"))?;
        let samples: Vec<f64> = (0..=16).map(|k| k as f64 / 16.0).collect();
        let points: Vec<occt::Vec3> = samples.iter().map(|&f| at(edge(index), f)).collect::<R<_>>()?;
        let in_plane = points.iter().all(|&p| dot(sub(p, points[0]), n).abs() < 1e-9);
        // Along a direction the profile doesn't lie square to, the chord less its part across the profile within the
        // plane of the direction and the profile's tangent, as normal to a profile's plane, the free end as with one
        // (batch 117 c004: within 0.06 mm² of Onshape's own fit).
        if !in_plane && !straight && end.tangent {
            let square = samples.iter().all(|&f| {
                edge(index).edge_tangent(f, false).is_ok_and(|t| dot(t.direction, n).abs() < 1e-9)
            });
            if !square {
                return Err(regen_error("BSURF_INCONSISTENT_CONDITIONS_ERROR"));
            }
            return Err(error("@opBoundarySurface: end conditions square to a direction aren't supported yet"));
        }
        if !straight && (end.magnitude - 1.0).abs() > 1e-12 {
            return Err(error("@opBoundarySurface: end conditions of magnitudes other than 1 aren't supported yet"));
        }
        if end.tangent && !in_plane {
            // Square to a direction given: the profile must lie square to it.
            let square = samples.iter().all(|&f| {
                edge(index).edge_tangent(f, false).is_ok_and(|t| dot(t.direction, n).abs() < 1e-9)
            });
            if !square {
                return Err(regen_error("BSURF_INCONSISTENT_CONDITIONS_ERROR"));
            }
        }
        normals[index] = Some((n, end.tangent, end.magnitude));
    }
    // The derivative leaving profile `index` at fraction `f`, the chord from the first profile to the second.
    let derivative = |index: usize, f: f64, chord: occt::Vec3| -> R<occt::Vec3> {
        let (n, tangent, magnitude) = normals[index].expect("a condition at this end");
        let along = edge(index).edge_tangent(fraction(index, f), false).map_err(occt_error)?.direction;
        let across = if tangent { Some(n) } else { normalize(cross(n, along)) };
        let across = across.ok_or_else(|| regen_error("CANNOT_MAKE_BSPLINESURFACE"))?;
        let d = sub(chord, scale(across, dot(chord, across)));
        if norm(d) <= 1e-6 * norm(chord).max(1e-9) {
            return Err(regen_error("CANNOT_MAKE_BSPLINESURFACE"));
        }
        Ok(scale(d, magnitude))
    };
    // With one end free between curved profiles, each curve across is the quadratic's, raised to a cubic, plus a
    // correction blended along the profiles from their ends: at each end of them, what the cubic reaching the free
    // profile along the chord (as between straight ones) differs from the quadratic by (batch 159 c005: from a line
    // matching a level sheet up to an arc whose ends are lower than its middle, 14v² − 5v³ high across the middle where
    // the quadratic is 9v²; nothing where the curves across at the ends are the quadratic's, batch 063 c011).
    let side = |f: f64| -> R<occt::Vec3> {
        let (p, q) = (at(&first, f)?, at(&second, fraction(1, f))?);
        let chord = sub(q, p);
        Ok(match (ends[0].is_some(), ends[1].is_some()) {
            (true, false) => scale(sub(chord, derivative(0, f, chord)?), 1.0 / 3.0),
            (false, true) => scale(sub(derivative(1, f, chord)?, chord), 1.0 / 3.0),
            _ => occt::Vec3::new(0.0, 0.0, 0.0),
        })
    };
    let one_free = !straight && ends.iter().filter(|e| e.is_some()).count() == 1;
    let zero = occt::Vec3::new(0.0, 0.0, 0.0);
    let (e0, e1) = if one_free { (side(0.0)?, side(1.0)?) } else { (zero, zero) };
    let count = 4;
    let rows = |f: f64| -> R<Vec<occt::Vec3>> {
        let (p, q) = (at(&first, f)?, at(&second, fraction(1, f))?);
        let chord = sub(q, p);
        if straight {
            let d0 = if ends[0].is_some() { derivative(0, f, chord)? } else { chord };
            let d1 = if ends[1].is_some() { derivative(1, f, chord)? } else { chord };
            return Ok(vec![p, add(p, scale(d0, 1.0 / 3.0)), sub(q, scale(d1, 1.0 / 3.0)), q]);
        }
        Ok(match (ends[0].is_some(), ends[1].is_some()) {
            (true, true) => {
                let (d0, d1) = (derivative(0, f, chord)?, derivative(1, f, chord)?);
                vec![p, add(p, scale(d0, 1.0 / 3.0)), sub(q, scale(d1, 1.0 / 3.0)), q]
            }
            (true, false) => {
                let d0 = derivative(0, f, chord)?;
                let blend = add(scale(e0, 1.0 - f), scale(e1, f));
                let middle = add(add(sub(q, scale(chord, 2.0 / 3.0)), scale(d0, 1.0 / 3.0)), blend);
                vec![p, add(p, scale(d0, 1.0 / 3.0)), middle, q]
            }
            _ => {
                let d1 = derivative(1, f, chord)?;
                let blend = add(scale(e0, 1.0 - f), scale(e1, f));
                let middle = add(add(add(p, scale(chord, 2.0 / 3.0)), scale(d1, -1.0 / 3.0)), blend);
                vec![p, middle, sub(q, scale(d1, 1.0 / 3.0)), q]
            }
        })
    };
    let curves = sampled_rows(count, rows)?;
    let nu = curves[0].poles.len();
    let poles: Vec<occt::Vec3> = (0..nu).flat_map(|i| curves.iter().map(move |c| c.poles[i])).collect();
    let (knots_v, degree_v): (Vec<f64>, i32) =
        if count == 4 { (vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0], 3) } else { (vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0], 2) };
    let face = occt::bspline_surface_face(&poles, (nu, count), &curves[0].knots, &knots_v, (curves[0].degree, degree_v))
        .map_err(|_| regen_error("BSURF_FAILED"))?;
    // A flat one is a plane, as Onshape's is (batch 121 c001, c002: lines in a plane).
    let face = occt::nurbs_converted(&face).map_err(occt_error)?;
    // Facing as the ruled surface between the profiles does.
    let facing = |s: &Shape| s.sub_shapes(ShapeKind::Face).first().and_then(|f| f.face_tangent_plane(0.5, 0.5).ok()).map(|t| t.normal);
    let face = match (facing(&face), facing(ruled)) {
        (Some(a), Some(b)) if dot(a, b) < 0.0 => face.reversed(),
        _ => face,
    };
    Ok(face)
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
        // By default the offset goes into the face, the other way flipped, except where the face is planar and
        // the edge's other face curved: then out of it (as if the edge's direction followed the curved face). So a
        // cylinder's top edge goes outwards across its top (batch 020 c008) but down its side (027 c001); a block's
        // top edge and a fillet's edge go into the face they're offset across (005 c009, 059 c006). Where the
        // offset leaves the face, Onshape makes no curve and no error (020 c008; 059 c004, c007).
        let face_shape = model.shape(face).clone();
        let target_planar = face_shape.face_plane().is_ok();
        let other_curved = model.bodies[face.body].faces.iter().any(|f| {
            !f.shape.is_same(&face_shape)
                && f.shape.sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(edge))
                && f.shape.face_plane().is_err()
        });
        let into_by_default = !(target_planar && other_curved);
        let outward = into_by_default == opposite;
        // A circle round a cone's or a sphere's axis offset across it: Onshape makes no curve (a cone's top circle,
        // batch 067 c001; a hemisphere's equator, 072 c007), though it does across a torus (067 c002).
        if let Ok(s) = face_shape.surface_definition()
            && (s.kind == 2 || s.kind == 3)
            && edge.curve_type().ok() == Some(occt::CurveType::Circle)
            && let Ok(c) = edge.curve_definition()
            && values::norm(values::cross(c.frame.normal, s.frame.normal)) < 1e-9
            && values::norm(values::cross(values::sub(c.frame.origin, s.frame.origin), s.frame.normal)) < 1e-7
        {
            continue;
        }
        // Across a B-spline face, along geodesics (batch 066 c004).
        if matches!(face_shape.surface_type(), Ok(occt::SurfaceType::BSpline | occt::SurfaceType::Bezier)) {
            if outward {
                continue;
            }
            if let Some(offset) = geodesic_offset(&face_shape, edge, distance)? {
                made.push((offset, face));
            }
            continue;
        }
        let signed = if outward { -distance } else { distance };
        let offset = match occt::offset_edge_on_planar_face(edge, &face_shape, signed) {
            Ok(offset) => offset,
            Err(_) if outward => continue,
            // Across other curved faces, along geodesics too (a torus's parallel, batch 067 c002).
            Err(_) if !target_planar => match geodesic_offset(&face_shape, edge, distance)? {
                Some(offset) => {
                    made.push((offset, face));
                    continue;
                }
                None => continue,
            },
            Err(_) => return Err(regen_error("OFFSET_CURVE_ON_FACE_FAILED")),
        };
        let mid = offset.edge_tangent(0.5, false).map_err(occt_error)?.origin;
        let off = occt::vertex(mid).and_then(|v| v.distance(&face_shape)).map(|d| d.distance).unwrap_or(0.0);
        if off > 1e-7 {
            continue;
        }
        // Extended, a straight offset runs on to the face's boundary (batch 027).
        let offset = if extend { extended_across(&offset, &face_shape)? } else { offset };
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
    if made.is_empty() {
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
    let mut solids = occt::enclose(&inputs).map_err(|_| regen_error("ENCLOSE_FAILED"))?;
    if solids.is_empty() {
        return Err(regen_error("ENCLOSE_NO_REGION"));
    }
    // Regions next to each other come out as one solid, the faces between them gone (batch 122 c016: a tube cut
    // through its axis is one part).
    if solids.len() > 1 {
        let fused = occt::boolean(occt::BooleanKind::Union, &solids[..1], &solids[1..], true)
            .map_err(|_| regen_error("ENCLOSE_FAILED"))?;
        solids = fused.shape().sub_shapes(ShapeKind::Solid);
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
                // At the edge's own point (the face's copy may run the other way, batch 067 c004).
                let reversed = used.is_reversed() != edge.is_reversed();
                let tangent = used.edge_tangent(if reversed { 1.0 - at } else { at }, true).map_err(occt_error)?;
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
                    && o.direction.is_some()
            }) {
                return Err(error("@opRuledSurface: along-a-vector overrides on curved edges aren't supported yet"));
            }
            let (w0, w1) = (width_at(a).unwrap_or(width), width_at(b).unwrap_or(width));
            let (a0, a1) = (angle_at(a).unwrap_or(angle), angle_at(b).unwrap_or(angle));
            let smooth = |t: f64| if cubic { t * t * (3.0 - 2.0 * t) } else { t };
            // The frame the rulings turn with: the rulings at angle 0 and a quarter turn, and the edge's tangent.
            let frame_at = |t: f64| -> R<[occt::Vec3; 3]> {
                let along = edge.edge_tangent(t, true).map_err(occt_error)?.direction;
                Ok([direction_at(edge, 0.0, t)?, direction_at(edge, std::f64::consts::FRAC_PI_2, t)?, along])
            };
            // Each end's ruling (an up-to point's from the vertex to it), held in that frame.
            let end_ruling = |t: f64, p: occt::Vec3, w: f64, angle: f64| -> R<[f64; 3]> {
                let v = match overrides.iter().find(|o| values::norm(values::sub(o.vertex, p)) < 1e-7).and_then(|o| o.up_to) {
                    Some(q) => values::sub(q, p),
                    None => values::scale(direction_at(edge, angle, t)?, w),
                };
                let f = frame_at(t)?;
                Ok([values::dot(v, f[0]), values::dot(v, f[1]), values::dot(v, f[2])])
            };
            let (r0, r1) = (end_ruling(0.0, a, w0, a0)?, end_ruling(1.0, b, w1, a1)?);
            // Onshape blends the ends' rulings as vectors in that frame (batch 139: normal at one end and tangent at
            // the other, its far edge shorter between them; up to a point at one end).
            let far_at = |t: f64| -> R<occt::Vec3> {
                let s = smooth(t);
                let point = edge.edge_tangent(t, true).map_err(occt_error)?.origin;
                let f = frame_at(t)?;
                let c: Vec<f64> = (0..3).map(|k| r0[k] + (r1[k] - r0[k]) * s).collect();
                Ok(values::add(
                    point,
                    values::add(values::add(values::scale(f[0], c[0]), values::scale(f[1], c[1])), values::scale(f[2], c[2])),
                ))
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
            // Rulings in one plane make a planar face; otherwise the sheet is ruled from the edge to the far curve
            // (normal to a cone from its top circle, a conical band, batch 067 c004).
            let base = values::sub(points[n], points[0]);
            let normal = values::normalize(values::cross(values::sub(b, a), base))
                .or_else(|| values::normalize(values::cross(values::sub(points[0], a), base)));
            let edge_points: Vec<occt::Vec3> = parameters
                .iter()
                .map(|t| edge.edge_tangent(*t, true).map(|l| l.origin))
                .collect::<Result<_, _>>()
                .map_err(occt_error)?;
            let planar = normal.is_some_and(|normal| {
                points.iter().chain(&edge_points).all(|p| values::dot(values::sub(*p, a), normal).abs() <= 1e-9)
            });
            if !planar {
                let closed = values::norm(values::sub(a, b)) < 1e-9;
                let far_edge = if closed {
                    let ring = &points[..n];
                    let mut lengths = vec![0.0];
                    for k in 0..ring.len() {
                        lengths.push(lengths[k] + values::norm(values::sub(ring[(k + 1) % ring.len()], ring[k])));
                    }
                    let m = ring.len();
                    let derivatives: Vec<occt::Vec3> = (0..m)
                        .map(|k| {
                            let span = if k == 0 { lengths[1] + lengths[m] - lengths[m - 1] } else { lengths[k + 1] - lengths[k - 1] };
                            values::scale(values::sub(ring[(k + 1) % m], ring[(k + m - 1) % m]), 1.0 / span)
                        })
                        .collect();
                    occt::interpolated_edge(ring, &derivatives, &lengths, true).map_err(occt_error)?
                } else {
                    far.edge()?
                };
                let wires: Vec<Shape> = [edge.clone(), far_edge]
                    .iter()
                    .map(|e| occt::connect_edges(std::slice::from_ref(e)).map_err(occt_error).map(|w| w.into_iter().next()))
                    .collect::<R<Option<Vec<_>>>>()?
                    .ok_or_else(|| regen_error("RULED_SURFACE_FAILED"))?;
                let sheet = occt::loft(&wires, false).map_err(|_| regen_error("RULED_SURFACE_FAILED"))?.shape();
                let mid = direction_at(edge, angle, 0.5)?;
                let at = edge.edge_tangent(0.5, true).map_err(occt_error)?;
                let facing = values::cross(mid, at.direction);
                for face in sheet.sub_shapes(ShapeKind::Face) {
                    let (fu, fv) = face.face_fraction(at.origin).map_err(occt_error)?;
                    let face_normal = face.face_tangent_plane(fu, fv).map_err(occt_error)?.normal;
                    sheets.push(if values::dot(face_normal, facing) < 0.0 { face.reversed() } else { face });
                }
                continue;
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
            let faces: Vec<Shape> = op.shape().sub_shapes(ShapeKind::Face).into_iter().map(|f| f.reversed()).collect();
            // Along a B-spline reference face, facing along the edge as that face's loop runs it × the ruling (batch
            // 066 c006); otherwise as OCCT's prism has it, reversed.
            let in_face = references
                .iter()
                .filter(|f| matches!(f.surface_type(), Ok(occt::SurfaceType::BSpline | occt::SurfaceType::Bezier)))
                .find_map(|f| f.sub_shapes(ShapeKind::Edge).into_iter().find(|e| e.is_same(edge)));
            match in_face {
                Some(used) => {
                    let mid = edge.edge_tangent(0.5, false).map_err(occt_error)?;
                    let along = if used.is_reversed() != edge.is_reversed() {
                        values::scale(mid.direction, -1.0)
                    } else {
                        mid.direction
                    };
                    let facing = values::cross(along, ruling);
                    for f in faces {
                        let (fu, fv) = f.face_fraction(mid.origin).map_err(occt_error)?;
                        let n = f.face_tangent_plane(fu, fv).map_err(occt_error)?.normal;
                        sheets.push(if values::dot(n, facing) < 0.0 { f.reversed() } else { f });
                    }
                }
                None => sheets.extend(faces),
            }
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
    // Trimmed to points (an offset curve's trim, batch 076 c005): each wire's edge kept between the points nearest
    // them.
    if trim && field(definition, "trimToPoints").as_array().is_some_and(|a| !a.is_empty()) {
        let points: Vec<occt::Vec3> = field(definition, "trimToPoints")
            .as_array()
            .map(|a| a.iter().map(|p| values::vec3(p, "trimToPoints")).collect::<R<Vec<_>>>())
            .transpose()?
            .unwrap_or_default();
        let [p0, p1] = points[..] else { return Err(error("@opMoveCurveBoundary: trims to other than two points aren't supported yet")) };
        let bodies = super::owner_bodies(model, field(definition, "wires"))?;
        let info = Info::new(id);
        let mut results = Vec::new();
        for b in bodies {
            let [edge] = &model.bodies[b].edges[..] else {
                return Err(error("@opMoveCurveBoundary: trimming wires of several edges to points isn't supported yet"));
            };
            let edge = edge.shape.clone();
            let (f0, f1) = (edge.edge_fraction(p0, false).map_err(occt_error)?, edge.edge_fraction(p1, false).map_err(occt_error)?);
            let (lo, hi) = if f0 <= f1 { (f0, f1) } else { (f1, f0) };
            let kept = occt::edge_between(&edge, lo, hi, false).map_err(|_| regen_error("TRIM_CURVE_FAILED"))?;
            results.push((b, kept));
        }
        for (b, edge) in results.into_iter().rev() {
            let old = model.bodies[b].clone();
            model.remove_bodies(&[b]);
            let wire = occt::connect_edges(&[edge]).map_err(occt_error)?.into_iter().next().ok_or_else(|| regen_error("TRIM_CURVE_FAILED"))?;
            model.add_body(NewBody::replacing(&old, wire), &Origins::default(), &info, &|_| None);
        }
        return Ok(Value::UNDEFINED);
    }
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
    // Onshape needs a help point to know which end to move (batch 079 c006: a vertex query finding none).
    let help = query::evaluate(model, field(definition, "helpPoint"))?;
    let Some(help) = help.first().map(|r| model.shape(*r).clone()) else {
        return Err(regen_error("MOVE_CURVE_BOUNDARY_SELECT_HELP_POINT"));
    };
    let help = Some(help);
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

/// A clamped, non-rational B-spline edge of a degree other than 3 (and more than 1), if it is one.
fn other_degree(edge: &Shape) -> Option<occt::BSplineData> {
    if !matches!(edge.curve_type().ok()?, occt::CurveType::BSpline | occt::CurveType::Bezier) {
        return None;
    }
    let data = edge.edge_bspline(1e-7, false, false).ok()?;
    let d = data.degree as usize;
    let clamped = |k: &[f64]| k.windows(2).all(|w| (w[1] - w[0]).abs() < 1e-12);
    // Weights all alike are no weights (an elevated curve's, batch 076 c003).
    let rational = data.rational && data.weights.iter().any(|w| (w - data.weights[0]).abs() > 1e-12);
    (d != 3 && d > 1 && !rational && !data.periodic && data.knots.len() == data.poles.len() + d + 1
        && clamped(&data.knots[..=d]) && clamped(&data.knots[data.knots.len() - d - 1..]))
        .then_some(data)
}

/// Whether an edge lies on a rational B-spline with weights that differ, other than a conic (one rational quadratic
/// span).
fn is_rational_spline(edge: &Shape) -> bool {
    edge.curve_type().ok() == Some(occt::CurveType::BSpline)
        && edge.edge_bspline(1e-7, false, false).is_ok_and(|data| {
            data.rational
                && data.weights.iter().any(|w| (w - data.weights[0]).abs() > 1e-12)
                && !(data.degree == 2 && data.poles.len() == 3)
        })
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
    // A conic (a sketch's, one rational quadratic span) becomes the clamped cubic spline through it, as a conic
    // fillet's section does, to within a micrometre (batch 070 c003).
    if data.rational && data.degree == 2 && data.poles.len() == 3 && data.weights.len() == 3 && !data.periodic {
        let (w0, w1, w2) = (data.weights[0], data.weights[1], data.weights[2]);
        let w = w1 / (w0 * w2).sqrt();
        let fitted = Cubic::rational_quadratic_within(
            data.poles[0],
            data.poles[1],
            data.poles[2],
            w,
            crate::curve_extend::CURVE_CONIC_TOLERANCE,
        )?;
        // Over the conic's own parameter range.
        let knots = fitted.knots.iter().map(|k| first + k * (last - first)).collect();
        let poles = fitted.poles;
        let cubic = Cubic { knots, poles };
        if norm(sub(*cubic.poles.last().expect("poles"), end.origin)) > 1e-9 || norm(sub(cubic.poles[0], start.origin)) > 1e-9
        {
            return Err(error("@opMoveCurveBoundary: B-splines running against their edge aren't supported yet"));
        }
        return Ok(cubic);
    }
    // Another rational B-spline becomes the clamped cubic spline through it, as a conic does (batch 195).
    let d = data.degree as usize;
    let rational = data.rational && data.weights.iter().any(|w| (w - data.weights[0]).abs() > 1e-12);
    if rational && !data.periodic && d >= 1 && data.knots.len() == data.poles.len() + d + 1
        && data.knots[..=d].iter().all(|k| *k == first)
        && data.knots[data.knots.len() - d - 1..].iter().all(|k| *k == last)
    {
        let cubic = Cubic::rational_within(d, &data.poles, &data.weights, &data.knots, crate::curve_extend::CURVE_CONIC_TOLERANCE)?;
        if norm(sub(*cubic.poles.last().expect("poles"), end.origin)) > 1e-9 || norm(sub(cubic.poles[0], start.origin)) > 1e-9
        {
            return Err(error("@opMoveCurveBoundary: B-splines running against their edge aren't supported yet"));
        }
        return Ok(cubic);
    }
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
        None => (cubic.clone(), 0.0),
    };
    let mut straight = distance - already;
    if straight <= 1e-12 {
        // Ending within its blend: the blend as far as the distance goes (batch 070 c004).
        let reach = |d: f64, blended: &crate::curve_extend::Cubic| {
            if by_chord {
                norm(sub(blended.derivative(blended.end(), 0), old_end))
            } else {
                blended.length(cubic.end(), cubic.end() + d)
            }
        };
        let full = base.end() - cubic.end();
        let (mut lo, mut hi) = (0.0, full);
        let mut cut = base.clone();
        for _ in 0..80 {
            let mid = (lo + hi) / 2.0;
            cut = cubic.blended_to(Some(mid))?.ok_or_else(|| error("curve extension: no blend"))?;
            if reach(mid, &cut) < distance {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        return Ok(if at_end { cut } else { cut.reversed() });
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

/// An edge of a face offset across it along geodesics (batch 066 c004: a loft's bottom edge offset up its B-spline
/// side): from points along the edge, walks of the distance into the face square to the edge, staying on its
/// surface; where a walk leaves the face the curve stops at its boundary. None if every walk leaves it.
fn geodesic_offset(face: &Shape, edge: &Shape, distance: f64) -> R<Option<Shape>> {
    use crate::values::{add, cross, dot, norm, normalize, scale, sub};
    const SAMPLES: usize = 48;
    const STEPS: usize = 512;
    let on_face = |x: occt::Vec3| -> Option<occt::Frame> {
        let (fu, fv) = face.face_fraction(x).ok()?;
        face.face_tangent_plane(fu.clamp(0.0, 1.0), fv.clamp(0.0, 1.0)).ok()
    };
    // The walk from the point at fraction t of the edge: its end, or None if it leaves the face. A walk along the
    // face's own boundary (a sphere band's meridian offset round its equator, batch 120 c001) stays on it.
    let walk = |t: f64| -> Option<Option<occt::Vec3>> {
        let a = edge.edge_tangent(t, false).ok()?;
        let start = on_face(a.origin)?;
        let mut dir = normalize(cross(start.normal, a.direction))?;
        // Into the face: of a short step either way, the one that stays nearer it (a step of a thousandth of the
        // distance off a revolved sheet's meridian can read as on it, batch 195 c011).
        let off = |d: occt::Vec3| {
            occt::vertex(add(a.origin, scale(d, distance * 1e-2)))
                .and_then(|v| v.distance(face))
                .map_or(f64::INFINITY, |d| d.distance)
        };
        if off(scale(dir, -1.0)) < off(dir) {
            dir = scale(dir, -1.0);
        }
        let h = distance / STEPS as f64;
        let mut x = a.origin;
        for _ in 0..STEPS {
            let y = add(x, scale(dir, h));
            // Past the face's edge the step projects back onto it, well short or onto the edge itself.
            let f = on_face(y)?;
            let (fu, fv) = face.face_fraction(y).ok()?;
            let at_edge = [fu, fv].iter().any(|t| *t <= 1e-9 || *t >= 1.0 - 1e-9);
            let along_edge = at_edge && norm(sub(f.origin, y)) < 1e-3 * h;
            if norm(sub(f.origin, y)) > 0.25 * h || (at_edge && !along_edge) {
                return Some(None);
            }
            let step = sub(f.origin, x);
            dir = normalize(sub(step, scale(f.normal, dot(step, f.normal))))?;
            x = f.origin;
        }
        Some(Some(x))
    };
    let mut ends: Vec<(f64, Option<occt::Vec3>)> = Vec::new();
    for k in 0..=SAMPLES {
        let t = k as f64 / SAMPLES as f64;
        ends.push((t, walk(t).ok_or_else(|| regen_error("OFFSET_CURVE_ON_FACE_FAILED"))?));
    }
    // Every walk the same straight move (an extruded sheet's end curve offset along it, batch 195 c009): the edge
    // itself moved so, as Onshape's is, the same curve.
    let moves: Option<Vec<occt::Vec3>> = ends
        .iter()
        .map(|(t, end)| Some(sub((*end)?, edge.edge_tangent(*t, false).ok()?.origin)))
        .collect();
    if let Some(moves) = moves.filter(|m| !m.is_empty()) {
        let v = moves[0];
        if moves.iter().all(|m| norm(sub(*m, v)) < 1e-9) && norm(v) > 1e-12 {
            let t = occt::Transform { m: [1.0, 0.0, 0.0, v.x, 0.0, 1.0, 0.0, v.y, 0.0, 0.0, 1.0, v.z] };
            let moved = occt::transform(edge, &t).map_err(occt_error)?.shape();
            let in_face = face.sub_shapes(ShapeKind::Edge).into_iter().find(|e| e.is_same(edge));
            let backwards = !face.is_reversed() && in_face.is_some_and(|e| e.is_reversed() != edge.is_reversed());
            return Ok(Some(if backwards { moved.reversed() } else { moved }));
        }
    }
    let first = ends.iter().position(|e| e.1.is_some());
    let last = ends.iter().rposition(|e| e.1.is_some());
    let (Some(first), Some(last)) = (first, last) else { return Ok(None) };
    // Where walks leave the face, the curve runs on to its boundary: the last walks' ends carried on in the face's
    // parameter plane, straight on from the last two, until they reach it (Onshape's ends lie on the face's edges,
    // batch 066 c004).
    let inner: Vec<occt::Vec3> = ends[first..=last].iter().filter_map(|e| e.1).collect();
    let to_boundary = |a: occt::Vec3, b: occt::Vec3, c: occt::Vec3| -> Option<occt::Vec3> {
        let uv = |p: occt::Vec3| face.face_fraction(p).ok();
        let (pa, pb, pc) = (uv(a)?, uv(b)?, uv(c)?);
        // Straight on from b through c (s = 0 at c, 1 a step further).
        let _ = pa;
        let at = |s: f64| (pc.0 + s * (pc.0 - pb.0), pc.1 + s * (pc.1 - pb.1));
        let outside = |s: f64| {
            let (u, v) = at(s);
            !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v)
        };
        let mut hi = 1.0;
        while !outside(hi) {
            hi *= 2.0;
            if hi > 64.0 {
                return None;
            }
        }
        let mut lo = 0.0;
        for _ in 0..60 {
            let m = (lo + hi) / 2.0;
            if outside(m) { hi = m } else { lo = m }
        }
        let (u, v) = at(lo);
        face.face_tangent_plane(u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)).ok().map(|f| f.origin)
    };
    let mut points: Vec<occt::Vec3> = Vec::new();
    if first > 0 && inner.len() >= 3 {
        points.extend(to_boundary(inner[2], inner[1], inner[0]));
    }
    points.extend(inner.iter().copied());
    if last < SAMPLES && inner.len() >= 3 {
        let n = inner.len();
        points.extend(to_boundary(inner[n - 3], inner[n - 2], inner[n - 1]));
    }
    points.dedup_by(|a, b| norm(sub(*a, *b)) < 1e-9);
    if points.len() < 2 {
        return Ok(None);
    }
    // Along the edge as the face's loop runs it (batch 066 c004).
    // Along the edge's own direction (batch 195 c009, c011, c012: an extruded sheet's top edge, a revolved sheet's
    // meridian and bottom circle), but against it where the face's loop runs the edge backwards on a face that isn't
    // itself reversed (batch 066 c004: a loft's bottom edge, which runs the other way in Onshape).
    let in_face = face.sub_shapes(ShapeKind::Edge).into_iter().find(|e| e.is_same(edge));
    if !face.is_reversed() && in_face.is_some_and(|e| e.is_reversed() != edge.is_reversed()) {
        points.reverse();
    }
    // A closed edge offset all round: a closed curve.
    let closed = first == 0 && last == SAMPLES && points.len() > 3 && norm(sub(points[0], *points.last().expect("points"))) < 1e-7;
    if closed {
        points.pop();
        let mut parameters = vec![0.0];
        for k in 0..points.len() {
            let next = points[(k + 1) % points.len()];
            parameters.push(parameters[k] + norm(sub(next, points[k])));
        }
        let n = points.len();
        let total = parameters[n];
        let derivatives: Vec<occt::Vec3> = (0..n)
            .map(|k| {
                let (a, b) = ((k + n - 1) % n, (k + 1) % n);
                let span = if k == 0 { parameters[1] + (total - parameters[n - 1]) } else { parameters[k + 1] - parameters[k - 1] };
                scale(sub(points[b], points[a]), 1.0 / span)
            })
            .collect();
        return occt::interpolated_edge(&points, &derivatives, &parameters, true).map(Some).map_err(occt_error);
    }
    let mut parameters = vec![0.0];
    for w in points.windows(2) {
        parameters.push(parameters.last().copied().unwrap_or(0.0) + norm(sub(w[1], w[0])));
    }
    let n = points.len();
    let derivatives: Vec<occt::Vec3> = (0..n)
        .map(|k| {
            let (a, b) = if k == 0 { (0, 1) } else if k == n - 1 { (n - 2, n - 1) } else { (k - 1, k + 1) };
            scale(sub(points[b], points[a]), 1.0 / (parameters[b] - parameters[a]))
        })
        .collect();
    occt::interpolated_edge(&points, &derivatives, &parameters, false).map(Some).map_err(occt_error)
}

/// How far along a ray from `p` along `d` it first meets a curve, found from samples along the curve.
fn ray_meets_curve(p: occt::Vec3, d: occt::Vec3, curve: &Shape) -> Option<f64> {
    use crate::values::{dot, norm, scale, sub};
    const SAMPLES: usize = 400;
    let point = |s: f64| curve.edge_tangent(s, false).ok().map(|a| a.origin);
    // The curve's distance from the ray's line.
    let off = |s: f64| {
        point(s).map_or(f64::MAX, |q| {
            let r = sub(q, p);
            norm(sub(r, scale(d, dot(r, d))))
        })
    };
    let h: Vec<f64> = (0..=SAMPLES).map(|i| off(i as f64 / SAMPLES as f64)).collect();
    let mut best: Option<f64> = None;
    for i in 0..=SAMPLES {
        let (before, after) = (if i > 0 { h[i - 1] } else { f64::MAX }, if i < SAMPLES { h[i + 1] } else { f64::MAX });
        if h[i] > before || h[i] > after {
            continue;
        }
        let (mut a, mut b) = ((i as f64 - 1.0).max(0.0) / SAMPLES as f64, (i as f64 + 1.0).min(SAMPLES as f64) / SAMPLES as f64);
        let g = (5f64.sqrt() - 1.0) / 2.0;
        for _ in 0..80 {
            let (c, e) = (b - g * (b - a), a + g * (b - a));
            if off(c) < off(e) { b = e } else { a = c }
        }
        let s = (a + b) / 2.0;
        if off(s) > 1e-7 {
            continue;
        }
        let t = dot(sub(point(s)?, p), d);
        if t > 1e-9 && best.is_none_or(|b| t < b) {
            best = Some(t);
        }
    }
    best
}

/// Where an edge meets another curve: the meeting nearest its end (or its start), found from samples along it.
fn curve_meeting(edge: &Shape, curve: &Shape, at_end: bool) -> Option<occt::Vec3> {
    let found = curve_meetings(edge, curve);
    let t = if at_end { found.into_iter().reduce(f64::max) } else { found.into_iter().reduce(f64::min) }?;
    edge.edge_tangent(t, false).ok().map(|a| a.origin)
}

/// The fractions (by parameter) along an edge where it meets another curve, inside its ends.
fn curve_meetings(edge: &Shape, curve: &Shape) -> Vec<f64> {
    const SAMPLES: usize = 400;
    let point = |t: f64| edge.edge_tangent(t, false).ok().map(|a| a.origin);
    let distance = |t: f64| {
        point(t).and_then(|p| occt::vertex(p).ok()).and_then(|v| v.distance(curve).ok()).map_or(f64::MAX, |d| d.distance)
    };
    let d: Vec<f64> = (0..=SAMPLES).map(|i| distance(i as f64 / SAMPLES as f64)).collect();
    let mut found = Vec::new();
    for i in 0..=SAMPLES {
        let (before, after) = (if i > 0 { d[i - 1] } else { f64::MAX }, if i < SAMPLES { d[i + 1] } else { f64::MAX });
        if d[i] > 1e-3 || d[i] > before || d[i] > after {
            continue;
        }
        // Golden section between the neighbouring samples.
        let (mut a, mut b) = ((i as f64 - 1.0).max(0.0) / SAMPLES as f64, (i as f64 + 1.0).min(SAMPLES as f64) / SAMPLES as f64);
        let g = (5f64.sqrt() - 1.0) / 2.0;
        for _ in 0..80 {
            let (c, e) = (b - g * (b - a), a + g * (b - a));
            if distance(c) < distance(e) { b = e } else { a = c }
        }
        let t = (a + b) / 2.0;
        if distance(t) < 1e-7 && t > 1e-9 && t < 1.0 - 1e-9 && !found.iter().any(|u: &f64| (u - t).abs() < 1e-9) {
            found.push(t);
        }
    }
    found
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
    let curved_limit = limit_curve.filter(|c| c.curve_type().ok() != Some(occt::CurveType::Line));
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
    // A closed chain has no end to extend (a Bézier's curve and control polygon, batch 071 c009).
    if ends.is_empty() && !trim {
        return Err(regen_error("MOVE_CURVE_BOUNDARY_EXTEND_REQUIRES_OPEN_CURVE"));
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
    // Trimmed by a curve: cut where they meet (an arc trimmed by a circle, batch 065), keeping the side the help
    // point is on; an arc stays an arc.
    if let (true, Some(curve), [_]) = (trim, curved_limit, edges) {
        let cut = curve_meeting(edge, curve, at_end).ok_or_else(|| regen_error("TRIM_CURVE_FAILED"))?;
        if circle {
            let centre = edge.curve_definition().map_err(occt_error)?.frame.origin;
            let (x, y, radius, angle) = arc_frame(centre, start.origin, start.direction, end.origin)?;
            let d = sub(cut, centre);
            let at = dot(d, y).atan2(dot(d, x)).rem_euclid(2.0 * std::f64::consts::PI);
            let (lo, hi) = if at_end { (at, angle) } else { (0.0, at) };
            let frame = occt::Frame { origin: centre, normal: cross(x, y), x };
            return occt::arc_edge(frame, radius, lo, hi).map_err(occt_error);
        }
        let cubic = as_cubic(edge, &start, &end)?;
        let (a, b) = (cubic.start(), cubic.end());
        let t = a + edge.edge_fraction(cut, false).map_err(occt_error)? * (b - a);
        let kept = if at_end { cubic.segment(t, b)? } else { cubic.segment(a, t)? };
        return kept.edge();
    }
    // A straight limit curve stands for the plane through it square to the curve's plane (an arc trimmed by a line,
    // batch 038).
    let limit = match (limit, limit_curve) {
        (Some(p), _) => Some(p),
        // Extended straight on up to a curve: the ray meets it (an arc up to a circle, batch 066 c003).
        (None, Some(_)) if curved_limit.is_some() => None,
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
            // A rational B-spline other than a conic stays itself, restricted where it crosses the plane (batch
            // 195 c007), the cubic through it only finding the crossing first.
            if is_rational_spline(edge) {
                let side = |f: f64| -> R<f64> {
                    let p = edge.edge_tangent(f, false).map_err(occt_error)?.origin;
                    Ok(dot(sub(p, plane.origin), plane.normal))
                };
                let guess = (cut - a) / (b - a);
                let (mut lo, mut hi) = ((guess - 0.02).max(0.0), (guess + 0.02).min(1.0));
                let low_side = side(lo)?;
                if low_side * side(hi)? > 0.0 {
                    return Err(regen_error("TRIM_CURVE_FAILED"));
                }
                for _ in 0..60 {
                    let mid = (lo + hi) / 2.0;
                    if (side(mid)? < 0.0) == (low_side < 0.0) {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                let f = (lo + hi) / 2.0;
                let (from, to) = if at_end { (f, 1.0) } else { (0.0, f) };
                return occt::edge_between(edge, from, to, false).map_err(|_| regen_error("TRIM_CURVE_FAILED"));
            }
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
        } else if let Some(curve) = curved_limit {
            ray_meets_curve(p, d, curve).ok_or_else(|| regen_error("TRIM_CURVE_FAILED"))?
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
    } else if let Some(data) = other_degree(edge) {
        // A B-spline of another degree, straight on (curving on too where it ends without curvature, as a fit spline
        // raised to degree 4 does, batch 076 c003, c004): as many evenly spaced poles as its degree, after a knot of
        // that multiplicity, as fast as the curve ends.
        let d = data.degree as usize;
        let (mut poles, mut knots) = (data.poles.clone(), data.knots.clone());
        if !at_end {
            poles.reverse();
            let (lo, hi) = (knots[0], *knots.last().expect("knots"));
            knots = knots.iter().rev().map(|k| lo + hi - k).collect();
        }
        let n = poles.len();
        let (u_end, u_before) = (*knots.last().expect("knots"), knots[knots.len() - d - 2]);
        let derivative = scale(sub(poles[n - 1], poles[n - 2]), d as f64 / (u_end - u_before));
        let speed = values::norm(derivative);
        let flat = {
            let (a, b, c) = (poles[n - 3], poles[n - 2], poles[n - 1]);
            values::norm(cross(sub(b, a), sub(c, b))) <= 1e-12 * values::norm(sub(c, a)).powi(2)
        };
        if curving && !flat {
            return Err(error("@opMoveCurveBoundary: curving extensions of B-splines of other degrees aren't supported yet"));
        }
        let direction = values::normalize(derivative).ok_or_else(|| regen_error("TRIM_CURVE_FAILED"))?;
        let length = straight(poles[n - 1], direction, 0.0)?;
        let end = poles[n - 1];
        knots.pop();
        knots.extend(std::iter::repeat_n(u_end + length / speed, d + 1));
        poles.extend((1..=d).map(|k| add(end, scale(direction, length * k as f64 / d as f64))));
        if !at_end {
            poles.reverse();
            let (lo, hi) = (knots[0], *knots.last().expect("knots"));
            knots = knots.iter().rev().map(|k| lo + hi - k).collect();
        }
        occt::bspline_edge(&poles, &[], &knots, d as i32, false).map_err(occt_error)?
    } else {
        // At its start, the curve is extended as its reverse is at its end, its own parameters kept (batch 038).
        let forward = as_cubic(edge, &start, &end)?;
        let cubic = if at_end { forward } else { forward.reversed() };
        let old_end = cubic.end();
        let (base, already) = match if curving { cubic.blended()? } else { None } {
            Some(blended) => {
                let length = blended.length(cubic.end(), blended.end());
                (blended, length)
            }
            None => (cubic.clone(), 0.0),
        };
        // Up to a curve met within the blend: the blend stops there (an arc extended with its curvature up to a
        // circle, batch 067 c003).
        let within_blend = match curved_limit {
            Some(curve) if already > 0.0 => {
                let edge = base.edge()?;
                let (a, b) = (base.start(), base.end());
                curve_meetings(&edge, curve)
                    .into_iter()
                    .map(|f| a + f * (b - a))
                    .filter(|t| *t > old_end + 1e-9)
                    .reduce(f64::min)
            }
            _ => None,
        };
        // A blind extension shorter than its blend: the blend as far as the distance (batch 070 c004).
        let blind = limit.is_none() && limit_surface.is_none() && curved_limit.is_none();
        let cut_blend = if within_blend.is_none() && blind && already > 0.0 && distance < already {
            let (mut lo, mut hi) = (0.0, base.end() - old_end);
            for _ in 0..80 {
                let mid = (lo + hi) / 2.0;
                if base.length(old_end, old_end + mid) < distance {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            Some(cubic.blended_to(Some((lo + hi) / 2.0))?.ok_or_else(|| regen_error("TRIM_CURVE_FAILED"))?)
        } else {
            None
        };
        let extended = match (within_blend, cut_blend) {
            (_, Some(cut)) => cut,
            (Some(t), None) => base.segment(base.start(), t)?,
            (None, None) => {
                let (p, d) = base.end_ray()?;
                base.straight_on(straight(p, d, already)?)?
            }
        };
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

/// `@opCreateBSplineSurface`: a sheet of a B-spline surface over its whole domain (batch 100 c002). Trimming
/// boundaries, periodic surfaces and solids aren't supported yet.
pub fn create_bspline_surface(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let surface = field(definition, "bSplineSurface");
    for key in ["boundaryBSplineCurves"] {
        if field(definition, key).as_array().is_some_and(|a| !a.is_empty()) {
            return Err(error(format!("@opCreateBSplineSurface: {key} isn't supported yet")));
        }
    }
    if values::opt_bool(definition, "makeSolid")?.unwrap_or(false) {
        return Err(error("@opCreateBSplineSurface: makeSolid isn't supported yet"));
    }
    if values::opt_bool(surface, "isUPeriodic")?.unwrap_or(false)
        || values::opt_bool(surface, "isVPeriodic")?.unwrap_or(false)
    {
        return Err(error("@opCreateBSplineSurface: periodic surfaces aren't supported yet"));
    }
    let degree = |key: &str| -> R<i32> {
        field(surface, key)
            .as_number()
            .map(|d| d as i32)
            .ok_or_else(|| error(format!("@opCreateBSplineSurface: {key}")))
    };
    let (u_degree, v_degree) = (degree("uDegree")?, degree("vDegree")?);
    let rows = field(surface, "controlPoints")
        .as_array()
        .ok_or_else(|| error("@opCreateBSplineSurface: controlPoints must be a matrix"))?;
    let mut poles = Vec::new();
    let mut nv = 0;
    for row in rows.iter() {
        let row = row.as_array().ok_or_else(|| error("@opCreateBSplineSurface: controlPoints must be a matrix"))?;
        nv = row.len();
        for p in row.iter() {
            poles.push(values::vec3(p, "opCreateBSplineSurface control point")?);
        }
    }
    let nu = rows.len();
    let knots_u = values::numbers(field(surface, "uKnots"), "opCreateBSplineSurface uKnots")?;
    let knots_v = values::numbers(field(surface, "vKnots"), "opCreateBSplineSurface vKnots")?;
    let face = if values::opt_bool(surface, "isRational")?.unwrap_or(false) {
        let mut weights = Vec::new();
        for row in field(surface, "weights").as_array().ok_or_else(|| error("@opCreateBSplineSurface: weights"))?.iter()
        {
            weights.extend(values::numbers(row, "opCreateBSplineSurface weights")?);
        }
        occt::rational_bspline_surface_face(&poles, &weights, (nu, nv), &knots_u, &knots_v, (u_degree, v_degree))
    } else {
        occt::bspline_surface_face(&poles, (nu, nv), &knots_u, &knots_v, (u_degree, v_degree))
    }
    .map_err(|_| crate::regen_error("CANNOT_MAKE_BSPLINESURFACE"))?;
    let info = Info::new(id);
    model.add_body(NewBody::new(face, BodyType::Sheet, info.clone()), &Origins::default(), &info, &|_| None);
    Ok(Value::UNDEFINED)
}
