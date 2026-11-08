//! `@opExtrude`: faces into solids and edges into sheets, with blind, up-to and through-all bounds.

use super::{ShapeSet, translation};
use crate::model::{BodyType, Cap, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, add, dot, field, scale, sub};
use crate::{occt_error, query, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Box3, Shape, ShapeKind, Vec3};
use std::rc::Rc;

/// How far an end of the extrude goes.
enum Bound {
    /// A distance along the direction (backwards for the start).
    Blind(f64),
    /// Up to a face or construction plane (the shape), offset along the direction.
    Surface(Shape, f64),
    /// Up to bodies, offset along the direction.
    Bodies(Vec<Shape>, f64),
    /// Past everything.
    ThroughAll,
}

fn bound(model: &Model, definition: &Value, which: &str, id: &FeatureId) -> R<Option<Bound>> {
    let key = |suffix: &str| format!("{which}{suffix}");
    let offset = match field(definition, &key("TranslationalOffset")) {
        v if v.is_undefined() => 0.0,
        v => values::length(v, "translational offset")?,
    };
    Ok(Some(match field(definition, &key("Bound")).as_str() {
        None if which == "start" => return Ok(None),
        Some("BLIND") => Bound::Blind(values::length(field(definition, &key("Depth")), "depth")?),
        Some("UP_TO_SURFACE") | Some("UP_TO_FACE") => {
            let found = query::evaluate(model, field(definition, &key("BoundEntity")))?;
            let face = found
                .iter()
                .find(|r| r.kind == EntityType::Face)
                .or_else(|| found.iter().find(|r| model.body(**r).faces.len() == 1))
                .ok_or_else(|| regen_error("EXTRUDE_SELECT_TERMINATING_SURFACE"))?;
            let shape = match face.kind {
                EntityType::Face => model.shape(*face).clone(),
                _ => model.body(*face).faces[0].shape.clone(),
            };
            Bound::Surface(shape, offset)
        }
        Some("UP_TO_BODY") => {
            let bodies = super::owner_bodies(model, field(definition, &key("BoundEntity")))?;
            Bound::Bodies(bodies.iter().map(|&b| model.bodies[b].shape.clone()).collect(), offset)
        }
        Some("UP_TO_NEXT") => {
            // Every solid in the context other than what this operation makes, whatever the boolean scope: Onshape
            // stops at the first face of any part. (Batch 004, c026, which starts inside another case's part.)
            let solids = model
                .bodies
                .iter()
                .filter(|b| b.body_type == BodyType::Solid && !b.construction && !b.info.is_created_by(id))
                .map(|b| b.shape.clone())
                .collect();
            Bound::Bodies(solids, offset)
        }
        Some("THROUGH_ALL") => Bound::ThroughAll,
        other => return Err(error(format!("@opExtrude: {which}Bound {other:?} isn't supported yet"))),
    }))
}

fn union_box(boxes: impl Iterator<Item = Box3>) -> Option<Box3> {
    boxes.filter(|b| !b.empty).reduce(|a, b| Box3 {
        min: Vec3::new(a.min.x.min(b.min.x), a.min.y.min(b.min.y), a.min.z.min(b.min.z)),
        max: Vec3::new(a.max.x.max(b.max.x), a.max.y.max(b.max.y), a.max.z.max(b.max.z)),
        empty: false,
    })
}

/// The farthest any body, or the profile, reaches along `direction` from `origin`, plus a margin.
fn reach(model: &Model, profile: &Shape, direction: Vec3) -> f64 {
    let frame = occt::Frame { origin: Vec3::new(0.0, 0.0, 0.0), normal: direction, x: values::canonical_x(direction) };
    let profile_box = profile.bounding_box_in(frame, false).ok();
    let all =
        union_box(model.bodies.iter().filter_map(|b| b.shape.bounding_box_in(frame, false).ok()).chain(profile_box));
    let (Some(all), Some(p)) = (all, profile_box) else { return 1.0 };
    (all.max.z - p.min.z).max(p.max.z - all.min.z).abs() + 1.0
}

/// The transient ids of the faces or edges an extrude sweeps.
fn profile_tids(model: &Model, definition: &Value) -> R<Vec<crate::model::Tid>> {
    let found = query::evaluate(model, field(definition, "entities"))?;
    let faces: Vec<_> = found.iter().filter(|r| r.kind == EntityType::Face).collect();
    let chosen: Vec<_> =
        if faces.is_empty() { found.iter().filter(|r| r.kind == EntityType::Edge).collect() } else { faces };
    Ok(chosen.into_iter().map(|r| model.tid(*r)).collect())
}

/// The faces or edges to extrude, as shapes, and whether they're faces.
fn profile(model: &Model, definition: &Value) -> R<(Vec<Shape>, bool)> {
    let found = query::evaluate(model, field(definition, "entities"))?;
    let faces: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Face).map(|r| model.shape(*r).clone()).collect();
    if !faces.is_empty() {
        let merged =
            if faces.len() == 1 { faces } else { vec![occt::merge_faces(&faces).map_err(occt_error)?.shape()] };
        return Ok((merged, true));
    }
    let edges: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Edge).map(|r| model.shape(*r).clone()).collect();
    if edges.is_empty() {
        return Err(regen_error("EXTRUDE_NO_SELECTED_REGION"));
    }
    Ok((occt::connect_edges(&edges).map_err(occt_error)?, false))
}

/// Keeps the pieces of `shape` that touch `start` (the profile at the start of the extrude).
pub(super) fn pieces_touching(shape: &Shape, start: &[Shape], kind: ShapeKind) -> Vec<Shape> {
    shape
        .sub_shapes(kind)
        .into_iter()
        .filter(|piece| start.iter().any(|s| piece.distance(s).is_ok_and(|d| d.distance < 1e-7)))
        .collect()
}

/// The faces of a split sheet on one side of a plane (that of `start`), sewn into shells: a split sheet comes back as
/// loose faces, which connect again across the cut (a hem's sheet cut at a slanted plane, batch 021).
fn sheet_faces_beside(shape: &Shape, plane: occt::Frame, start: &[Shape]) -> R<Vec<Shape>> {
    let side = |p: Vec3| dot(sub(p, plane.origin), plane.normal);
    let reference = start.iter().find_map(|s| s.centroid().ok()).map(side).unwrap_or(0.0);
    let faces: Vec<Shape> = shape
        .sub_shapes(ShapeKind::Face)
        .into_iter()
        .filter(|f| f.centroid().is_ok_and(|c| side(c) * reference > 0.0))
        .collect();
    if faces.is_empty() {
        return Ok(Vec::new());
    }
    let sewn = occt::sew_faces(&faces, 1e-7).map_err(occt_error)?;
    let shells = sewn.sub_shapes(ShapeKind::Shell);
    Ok(if shells.is_empty() { vec![sewn] } else { shells })
}

pub fn extrude(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let (profiles, solid) = profile(model, definition)?;
    let direction = values::normalize(values::vec3(field(definition, "direction"), "opExtrude direction")?)
        .ok_or_else(|| error("@opExtrude: zero direction"))?;
    let end = bound(model, definition, "end", &id)?.expect("an end bound");
    let start = bound(model, definition, "start", &id)?;
    let reach = reach(model, &profiles[0], direction);

    // Where the sweep starts and how far it goes before any trimming to a bound. The start is `startDepth` behind
    // the profile, or forward when `isStartBoundOpposite` is false (std's symmetric extrude passes a negative
    // depth that way).
    let opposite = values::opt_bool(definition, "isStartBoundOpposite")?.unwrap_or(true);
    let start_at = match &start {
        None => 0.0,
        Some(Bound::Blind(d)) => {
            if opposite {
                -d
            } else {
                *d
            }
        }
        Some(Bound::ThroughAll) => -reach,
        // Up to a plane: swept from far behind, then cut at the plane (a hem's step, batch 021).
        Some(Bound::Surface(face, _)) if face.face_plane().is_ok() => -reach,
        Some(_) => return Err(error("@opExtrude: up-to start bounds other than planes aren't supported yet")),
    };
    let end_at = match &end {
        Bound::Blind(d) => *d,
        _ => reach,
    };
    let length = end_at - start_at;
    if length <= 1e-8 {
        return Err(regen_error("EXTRUDE_FAILED"));
    }

    let dependency: Option<Rc<[crate::model::Tid]>> = Some(profile_tids(model, definition)?.into());
    let created = Rc::new(Info {
        created_by: id.clone(),
        sketch_entity: None,
        cap: None,
        dependency: dependency.clone(),
        ..Default::default()
    });
    let start_info = Rc::new(Info {
        created_by: id.clone(),
        sketch_entity: None,
        cap: Some(Cap::Start),
        dependency: dependency.clone(),
        ..Default::default()
    });
    let end_info = Rc::new(Info {
        created_by: id.clone(),
        sketch_entity: None,
        cap: Some(Cap::End),
        dependency,
        ..Default::default()
    });
    let mut made: Vec<(Shape, BodyType)> = Vec::new();
    let (mut start_cap, mut end_cap) = (ShapeSet::default(), ShapeSet::default());
    for profile in &profiles {
        let moved = if start_at == 0.0 {
            profile.clone()
        } else {
            occt::transform(profile, &translation(scale(direction, start_at))).map_err(occt_error)?.shape()
        };
        let op = occt::prism(&moved, scale(direction, length)).map_err(|_| regen_error("EXTRUDE_FAILED"))?;
        let first = op.first();
        for face in &first {
            start_cap.insert_all(face);
        }
        let kind = if solid { ShapeKind::Solid } else { ShapeKind::Shell };
        let swept = op.shape();
        let result = match &end {
            Bound::Blind(_) | Bound::ThroughAll => {
                for face in op.last() {
                    end_cap.insert_all(&face);
                }
                swept.sub_shapes(kind)
            }
            Bound::Surface(face, offset) => {
                let face = if *offset == 0.0 {
                    face.clone()
                } else {
                    occt::transform(face, &translation(scale(direction, *offset))).map_err(occt_error)?.shape()
                };
                let cutter = match face.face_plane() {
                    Ok(plane) => occt::plane_face(plane, -2.0 * reach, 2.0 * reach, -2.0 * reach, 2.0 * reach)
                        .map_err(occt_error)?,
                    Err(_) => face,
                };
                let split = occt::split(std::slice::from_ref(&swept), std::slice::from_ref(&cutter))
                    .map_err(|_| regen_error("EXTRUDE_FAILED"))?;
                let starts = moved_start(&first, &moved);
                let kept = match (solid, cutter.face_plane()) {
                    (false, Ok(plane)) => sheet_faces_beside(&split.shape(), plane, &starts)?,
                    _ => pieces_touching(&split.shape(), &starts, kind),
                };
                for piece in &kept {
                    for f in piece.sub_shapes(ShapeKind::Face) {
                        if f.distance(&cutter).is_ok_and(|d| d.distance < 1e-7) && on_surface(&f, &cutter) {
                            end_cap.insert_all(&f);
                        }
                    }
                }
                kept
            }
            Bound::Bodies(bodies, offset) => {
                let bodies: Vec<Shape> = if *offset == 0.0 {
                    bodies.clone()
                } else {
                    let t = translation(scale(direction, *offset));
                    bodies
                        .iter()
                        .map(|b| occt::transform(b, &t).map(|op| op.shape()))
                        .collect::<Result<_, _>>()
                        .map_err(occt_error)?
                };
                // Only bodies the sweep can reach matter.
                let sweep_box = swept.bounding_box(false).map_err(occt_error)?;
                let bodies: Vec<Shape> = bodies
                    .into_iter()
                    .filter(|b| b.bounding_box(false).is_ok_and(|bb| boxes_meet(&bb, &sweep_box)))
                    .collect();
                // Split the sweep by the bodies and keep the piece at the profile: it ends at the first face it
                // meets, even if it starts inside a body.
                let split =
                    occt::split(std::slice::from_ref(&swept), &bodies).map_err(|_| regen_error("EXTRUDE_FAILED"))?;
                let kept = pieces_touching(&split.shape(), &moved_start(&first, &moved), kind);
                for piece in &kept {
                    for f in piece.sub_shapes(ShapeKind::Face) {
                        if bodies.iter().any(|b| on_surface(&f, b)) {
                            end_cap.insert_all(&f);
                        }
                    }
                }
                kept
            }
        };
        // A planar start bound: keep what's ahead of the plane along the direction; its faces there are the start cap.
        let result = match &start {
            Some(Bound::Surface(face, offset)) => {
                let mut plane = face.face_plane().map_err(occt_error)?;
                plane.origin = add(plane.origin, scale(direction, *offset));
                let cutter =
                    occt::plane_face(plane, -2.0 * reach, 2.0 * reach, -2.0 * reach, 2.0 * reach).map_err(occt_error)?;
                let split = occt::split(&result, std::slice::from_ref(&cutter)).map_err(|_| regen_error("EXTRUDE_FAILED"))?;
                let is_ahead = |piece: &Shape| {
                    piece.centroid().is_ok_and(|c| dot(sub(c, plane.origin), plane.normal) * dot(direction, plane.normal) > 0.0)
                };
                // Sheets come back as loose faces: those ahead, sewn together again.
                let ahead: Vec<Shape> = if solid {
                    split.shape().sub_shapes(kind).into_iter().filter(|p| is_ahead(p)).collect()
                } else {
                    let faces: Vec<Shape> = split.shape().sub_shapes(ShapeKind::Face).into_iter().filter(|f| is_ahead(f)).collect();
                    if faces.is_empty() {
                        Vec::new()
                    } else {
                        let sewn = occt::sew_faces(&faces, 1e-7).map_err(occt_error)?;
                        let shells = sewn.sub_shapes(ShapeKind::Shell);
                        if shells.is_empty() { vec![sewn] } else { shells }
                    }
                };
                for piece in &ahead {
                    for f in piece.sub_shapes(ShapeKind::Face) {
                        if on_surface(&f, &cutter) {
                            start_cap.insert_all(&f);
                        }
                    }
                }
                ahead
            }
            _ => result,
        };
        if result.is_empty() {
            return Err(regen_error("EXTRUDE_FAILED"));
        }
        // A sheet swept from edges faces along tangent × direction in Onshape, the reverse of OCCT's prism.
        let result = if solid { result } else { result.iter().map(|s| s.reversed()).collect() };
        made.extend(result.into_iter().map(|s| (s, if solid { BodyType::Solid } else { BodyType::Sheet })));
    }
    // Cylinders swept from circles get Onshape's coordinate system.
    let circles = super::frames::circles(&profiles);
    let side_info = |frame| {
        Rc::new(Info {
            surface_frame: Some(frame),
            created_by: created.created_by.clone(),
            dependency: created.dependency.clone(),
            ..Default::default()
        })
    };
    let special = |s: &Shape| {
        if end_cap.contains(s) {
            Some(end_info.clone())
        } else if start_cap.contains(s) {
            Some(start_info.clone())
        } else if s.kind() == Some(ShapeKind::Face) && !circles.is_empty() {
            super::frames::extruded(s, &circles, direction).map(side_info)
        } else {
            None
        }
    };
    let origins = swept_origins(model, definition, &made, direction)?;
    for (shape, body_type) in made {
        model.add_body(NewBody::new(shape, body_type, created.clone()), &origins, &created, &special);
    }
    Ok(Value::UNDEFINED)
}

/// Where an extrusion's faces and edges came from, for tracking: side faces from the profile edges swept, edges
/// along the direction from the profile vertices swept, caps and their edges from the profile's faces and edges
/// (std's hem tracks a sketch arc's end point into the side edge it sweeps).
fn swept_origins(model: &Model, definition: &Value, made: &[(Shape, BodyType)], direction: Vec3) -> R<Origins> {
    use crate::values::{add, cross, dot, norm, scale, sub};
    let mut sources: Vec<crate::model::Entity> = Vec::new();
    for r in query::evaluate(model, field(definition, "entities"))? {
        let shape = model.shape(r).clone();
        for entity in model.bodies[r.body].all_entities() {
            let kind = entity.shape.kind();
            let inside = entity.shape.is_same(&shape)
                || kind.is_some_and(|k| shape.sub_shapes(k).iter().any(|x| x.is_same(&entity.shape)));
            if inside && !sources.iter().any(|e| e.tid == entity.tid) {
                sources.push(entity.clone());
            }
        }
    }
    let on =
        |p: Vec3, target: &Shape| occt::vertex(p).and_then(|v| v.distance(target)).is_ok_and(|d| d.distance < 1e-8);
    let samples = |s: &Shape| -> Vec<Vec3> {
        [0.0, 0.5, 1.0].iter().filter_map(|&t| s.edge_tangent(t, false).ok().map(|a| a.origin)).collect()
    };
    let mut origins = Origins::default();
    for (shape, _) in made {
        for edge in shape.sub_shapes(ShapeKind::Edge) {
            let along = edge.curve_type().ok() == Some(occt::CurveType::Line)
                && edge.edge_tangent(0.5, false).is_ok_and(|a| norm(cross(a.direction, direction)) < 1e-9);
            let points = samples(&edge);
            let Some(&mid) = points.get(1) else { continue };
            for source in &sources {
                match source.shape.kind() {
                    Some(ShapeKind::Vertex) if along => {
                        let Ok(v) = source.shape.vertex_point() else { continue };
                        let d = sub(v, mid);
                        if norm(sub(d, scale(direction, dot(d, direction)))) < 1e-9 {
                            origins.derive(&edge, source, false);
                        }
                    }
                    Some(ShapeKind::Edge) if !along => {
                        let from = samples(&source.shape);
                        let Some(&smid) = from.get(1) else { continue };
                        let t = dot(sub(mid, smid), direction);
                        if from.iter().all(|p| on(add(*p, scale(direction, t)), &edge)) {
                            origins.derive(&edge, source, true);
                        }
                    }
                    _ => {}
                }
            }
        }
        for face in shape.sub_shapes(ShapeKind::Face) {
            let Ok(p) = face.face_inner_point() else { continue };
            for source in &sources {
                match source.shape.kind() {
                    Some(ShapeKind::Edge) => {
                        let from = samples(&source.shape);
                        let Some(&smid) = from.get(1) else { continue };
                        let t = dot(sub(p, smid), direction);
                        if from.iter().all(|q| on(add(*q, scale(direction, t)), &face)) {
                            origins.derive(&face, source, false);
                        }
                    }
                    Some(ShapeKind::Face) => {
                        let Ok(q) = source.shape.face_inner_point() else { continue };
                        let t = dot(sub(p, q), direction);
                        if on(add(q, scale(direction, t)), &face) && on(sub(p, scale(direction, t)), &source.shape) {
                            origins.derive(&face, source, true);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(origins)
}

fn boxes_meet(a: &Box3, b: &Box3) -> bool {
    a.min.x <= b.max.x
        && b.min.x <= a.max.x
        && a.min.y <= b.max.y
        && b.min.y <= a.max.y
        && a.min.z <= b.max.z
        && b.min.z <= a.max.z
}

/// The start of the sweep: the prism's first faces, or for sheets, the moved profile's edges.
fn moved_start(first: &[Shape], moved: &Shape) -> Vec<Shape> {
    if first.is_empty() { vec![moved.clone()] } else { first.to_vec() }
}

/// Whether a face lies on the cutting surface (sampled at its middle).
pub(super) fn on_surface(face: &Shape, cutter: &Shape) -> bool {
    face.face_tangent_plane(0.5, 0.5)
        .is_ok_and(|p| occt::vertex(p.origin).is_ok_and(|v| v.distance(cutter).is_ok_and(|d| d.distance < 1e-7)))
}
