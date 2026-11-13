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

/// How far a through-all extrude goes: as Onshape, the diagonal of the box round the parts (not construction or
/// sketch bodies: batch 007's sketched 100 mm axis line doesn't count) and the profile, plus 0.01 mm (batch 007
/// c007: a hole's cylinder has its origin at the far end, 2100.98596 mm from its profile, the box round the batch's
/// parts so far being 2100.97596 across). From anywhere in the box, that reaches past it.
fn reach(model: &Model, profile: &Shape) -> f64 {
    let parts = model
        .bodies
        .iter()
        .filter(|b| !b.construction && b.sketch_plane.is_none() && b.frame.is_none())
        .filter_map(|b| b.shape.bounding_box(true).ok());
    let Some(all) = union_box(parts.chain(profile.bounding_box(true).ok())) else { return 1.0 };
    values::norm(values::sub(all.max, all.min)) + 1e-5
}

/// The transient ids of the faces or edges an extrude sweeps.
fn profile_tids(model: &Model, definition: &Value) -> R<Vec<crate::model::Tid>> {
    let found = query::evaluate(model, field(definition, "entities"))?;
    let faces: Vec<_> = found.iter().filter(|r| r.kind == EntityType::Face).collect();
    let chosen: Vec<_> =
        if faces.is_empty() { found.iter().filter(|r| r.kind == EntityType::Edge).collect() } else { faces };
    Ok(chosen.into_iter().map(|r| model.tid(*r)).collect())
}

/// The faces or edges to extrude, as shapes, and whether they're faces. Moved first by the definition's `transform`,
/// a feature pattern instance's (batch 026).
fn profile(model: &Model, definition: &Value) -> R<(Vec<Shape>, bool)> {
    let (shapes, solid) = untransformed_profile(model, definition)?;
    match crate::pattern_stack::transform_of(definition)? {
        None => Ok((shapes, solid)),
        Some(t) => Ok((
            shapes
                .iter()
                .map(|s| occt::transform(s, &t).map(|op| op.shape()))
                .collect::<Result<_, _>>()
                .map_err(occt_error)?,
            solid,
        )),
    }
}

fn untransformed_profile(model: &Model, definition: &Value) -> R<(Vec<Shape>, bool)> {
    let found = query::evaluate(model, field(definition, "entities"))?;
    let faces: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Face).map(|r| model.shape(*r).clone()).collect();
    if !faces.is_empty() {
        let merged =
            if faces.len() == 1 { faces } else { vec![occt::merge_faces(&faces).map_err(occt_error)?.shape()] };
        // Collinear lines along a region's boundary sweep one side face in Onshape, not one each (batch 087 c001:
        // the extrude's cap has the merged edges); faces without any keep their own edges.
        let merged = merged
            .into_iter()
            .map(|face| {
                let count = |s: &Shape| s.sub_shapes(occt::ShapeKind::Edge).len();
                match occt::merge_edges(&face) {
                    Ok(op) if count(&op.shape()) < count(&face) => op.shape(),
                    _ => face,
                }
            })
            .collect();
        return Ok((merged, true));
    }
    let edges: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Edge).map(|r| model.shape(*r).clone()).collect();
    if edges.is_empty() {
        return Err(regen_error("EXTRUDE_NO_SELECTED_REGION"));
    }
    // Lines in a row along one line sweep one face, as a region's do (batch 166 c001: a sheet from two such lines).
    let wires = occt::connect_edges(&edges).map_err(occt_error)?;
    Ok((wires.into_iter().map(|w| collinear_merged(&w).unwrap_or(w)).collect(), false))
}

/// A wire with each run of lines along one line, end to end, made one line; `None` if it has no such run.
fn collinear_merged(wire: &Shape) -> Option<Shape> {
    let edges = wire.sub_shapes(ShapeKind::Edge);
    let line = |e: &Shape| -> Option<(Vec3, Vec3)> {
        (e.curve_type().ok() == Some(occt::CurveType::Line))
            .then(|| Some((e.edge_tangent(0.0, false).ok()?.origin, e.edge_tangent(1.0, false).ok()?.origin)))
            .flatten()
    };
    let mut out: Vec<Shape> = Vec::new();
    let mut run: Option<(Vec3, Vec3)> = None;
    let mut merged_any = false;
    let flush = |run: &mut Option<(Vec3, Vec3)>, out: &mut Vec<Shape>| -> Option<()> {
        if let Some((a, b)) = run.take() {
            out.push(occt::line_edge(a, b).ok()?);
        }
        Some(())
    };
    for e in &edges {
        match (line(e), run) {
            (Some((a, b)), Some((ra, rb))) => {
                let d = crate::values::normalize(sub(rb, ra))?;
                let e_dir = crate::values::normalize(sub(b, a))?;
                let joined = crate::values::norm(sub(a, rb)) < 1e-9 && crate::values::norm(crate::values::cross(d, e_dir)) < 1e-9 && dot(d, e_dir) > 0.0;
                if joined {
                    run = Some((ra, b));
                    merged_any = true;
                    continue;
                }
                flush(&mut run, &mut out)?;
                run = Some((a, b));
            }
            (Some((a, b)), None) => {
                run = Some((a, b));
            }
            (None, _) => {
                flush(&mut run, &mut out)?;
                out.push(e.clone());
            }
        }
    }
    flush(&mut run, &mut out)?;
    if !merged_any {
        return None;
    }
    occt::ordered_wire(&out).ok()
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
    // Up to next passes by parts the profile starts inside (batch 148 c002: a column on its own base, starting inside
    // another case's plate, runs on up to its own roof); a part the profile only touches stops it (batch 004 c026).
    let end = match end {
        Bound::Bodies(solids, offset) if field(definition, "endBound").as_str() == Some("UP_TO_NEXT") => {
            let inside: Vec<Vec3> = profiles
                .iter()
                .filter_map(|p| match solid {
                    true => p.face_inner_point().ok(),
                    false => p.edge_tangent(0.5, false).ok().map(|a| a.origin),
                })
                .collect();
            let kept = solids.into_iter().filter(|s| !inside.iter().any(|&q| s.contains_point(q).unwrap_or(false)));
            Bound::Bodies(kept.collect(), offset)
        }
        other => other,
    };
    let start = bound(model, definition, "start", &id)?;
    let reach = reach(model, &profiles[0]);

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
        // Up to a plane: swept from far behind, then cut at the plane (a hem's step, batch 021); up to a curved face or
        // bodies, of a solid, the same (batch 164 c004, c005).
        Some(Bound::Surface(face, _)) if face.face_plane().is_ok() => -reach,
        Some(Bound::Surface(..) | Bound::Bodies(..)) if solid => -reach,
        Some(_) => return Err(error("@opExtrude: up-to start bounds of sheets other than planes aren't supported yet")),
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
                // Where the sweep meets a curved bound, OCCT leaves the curve in pieces; Onshape's is one edge
                // (batch 124 c003).
                let kept: Vec<Shape> = kept.into_iter().map(|p| occt::edges_fused(&p).unwrap_or(p)).collect();
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
            Some(Bound::Surface(face, offset)) if face.face_plane().is_ok() => {
                let mut plane = face.face_plane().map_err(occt_error)?;
                plane.origin = add(plane.origin, scale(direction, *offset));
                let cutter = occt::plane_face(plane, -2.0 * reach, 2.0 * reach, -2.0 * reach, 2.0 * reach)
                    .map_err(occt_error)?;
                let split =
                    occt::split(&result, std::slice::from_ref(&cutter)).map_err(|_| regen_error("EXTRUDE_FAILED"))?;
                let is_ahead = |piece: &Shape| {
                    piece
                        .centroid()
                        .is_ok_and(|c| dot(sub(c, plane.origin), plane.normal) * dot(direction, plane.normal) > 0.0)
                };
                // Sheets come back as loose faces: those ahead, sewn together again.
                let ahead: Vec<Shape> = if solid {
                    split.shape().sub_shapes(kind).into_iter().filter(|p| is_ahead(p)).collect()
                } else {
                    let faces: Vec<Shape> =
                        split.shape().sub_shapes(ShapeKind::Face).into_iter().filter(|f| is_ahead(f)).collect();
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
            // Up to a curved face or bodies: keep the piece the profile is in; its faces on the bound are the start cap.
            Some(Bound::Surface(face, offset)) => {
                let cutter = if *offset == 0.0 {
                    face.clone()
                } else {
                    occt::transform(face, &translation(scale(direction, *offset))).map_err(occt_error)?.shape()
                };
                let split =
                    occt::split(&result, std::slice::from_ref(&cutter)).map_err(|_| regen_error("EXTRUDE_FAILED"))?;
                let kept: Vec<Shape> = pieces_touching(&split.shape(), std::slice::from_ref(profile), kind)
                    .into_iter()
                    .map(|p| occt::edges_fused(&p).unwrap_or(p))
                    .collect();
                for piece in &kept {
                    for f in piece.sub_shapes(ShapeKind::Face) {
                        if f.distance(&cutter).is_ok_and(|d| d.distance < 1e-7) && on_surface(&f, &cutter) {
                            start_cap.insert_all(&f);
                        }
                    }
                }
                kept
            }
            Some(Bound::Bodies(bodies, offset)) => {
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
                let split = occt::split(&result, &bodies).map_err(|_| regen_error("EXTRUDE_FAILED"))?;
                let kept = pieces_touching(&split.shape(), std::slice::from_ref(profile), kind);
                for piece in &kept {
                    for f in piece.sub_shapes(ShapeKind::Face) {
                        if bodies.iter().any(|b| on_surface(&f, b)) {
                            start_cap.insert_all(&f);
                        }
                    }
                }
                kept
            }
            _ => result,
        };
        if result.is_empty() {
            return Err(regen_error("EXTRUDE_FAILED"));
        }
        // A sheet swept from edges faces along tangent × direction in Onshape, the reverse of OCCT's prism.
        let result = if solid { result } else { result.iter().map(|s| s.reversed()).collect() };
        // A sheet swept from a straight curve that isn't a line (a spline through points in a row) is a plane in
        // Onshape, its edges kept (batch 163 c007–c009).
        let result: Vec<Shape> = if solid { result } else { result.iter().map(flattened).collect() };
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
            super::frames::extruded(s, &circles, direction, !solid).map(side_info)
        } else if s.kind() == Some(ShapeKind::Edge) && s.curve_type().ok() == Some(occt::CurveType::Line) {
            // An edge swept from a vertex: Onshape's line sits at its far end (batch 047).
            let (a, b) = (s.edge_tangent(0.0, false).ok()?, s.edge_tangent(1.0, false).ok()?);
            if crate::values::norm(crate::values::cross(a.direction, direction)) > 1e-9 {
                return None;
            }
            let far = if crate::values::dot(crate::values::sub(b.origin, a.origin), direction) > 0.0 {
                b.origin
            } else {
                a.origin
            };
            let near = if far == b.origin { a.origin } else { b.origin };
            // A solid's runs from the far end back (batch 088), a sheet's out from the profile (batch 159 c012).
            let sense = if solid {
                crate::values::normalize(crate::values::sub(near, far)).map(|d| (far, d))
            } else {
                crate::values::normalize(crate::values::sub(far, near)).map(|d| (near, d))
            };
            Some(Rc::new(Info {
                created_by: created.created_by.clone(),
                dependency: created.dependency.clone(),
                line_origin: Some(far),
                sense,
                ..Default::default()
            }))
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

/// A one-face sheet whose face is flat but not a plane, rebuilt on the plane through its edges, facing as it did.
fn flattened(sheet: &Shape) -> Shape {
    let faces = sheet.sub_shapes(ShapeKind::Face);
    let [face] = &faces[..] else { return sheet.clone() };
    if face.face_plane().is_ok() {
        return sheet.clone();
    }
    let grid: Option<Vec<occt::Frame>> =
        [0.0, 0.5, 1.0].iter().flat_map(|&u| [0.0, 0.5, 1.0].map(|v| face.face_tangent_plane(u, v).ok())).collect();
    let Some(grid) = grid else { return sheet.clone() };
    let n = grid[0].normal;
    let flat = grid.iter().all(|t| {
        crate::values::norm(crate::values::cross(t.normal, n)) < 1e-9
            && crate::values::dot(crate::values::sub(t.origin, grid[0].origin), n).abs() < 1e-7
    });
    if !flat {
        return sheet.clone();
    }
    let Ok(plane) = occt::fill_edges(&face.sub_shapes(ShapeKind::Edge)) else { return sheet.clone() };
    let facing = plane.sub_shapes(ShapeKind::Face).first().and_then(|f| f.face_tangent_plane(0.5, 0.5).ok()).map(|t| t.normal);
    match facing {
        Some(m) if crate::values::dot(m, n) < 0.0 => plane.reversed(),
        Some(_) => plane,
        None => sheet.clone(),
    }
}
