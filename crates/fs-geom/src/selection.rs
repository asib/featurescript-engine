//! Selection queries: faces flood-filled across convex, concave or bounded edges, the faces of holes and fillets,
//! loops of edges, parallel edges, matching entities, planes by normal, entities in front of a plane, and edges by
//! convexity. Checked against Onshape in batch 007 (c008–c011).

use std::collections::BTreeSet;

use crate::model::{EntityRef, EntityType, Model};
use crate::values;
use fs_interp::eval::R;
use occt::{Shape, ShapeKind, Vec3};

/// How a body's two faces meet along an edge (`EdgeConvexityType`). With `by_curvature`, smooth stretches count as
/// convex or concave by how the faces bend, as the flood-filling queries count a fillet's edges.
pub fn convexity(model: &Model, edge: EntityRef, by_curvature: bool) -> Option<&'static str> {
    let kind = model.bodies[edge.body].shape.edge_convexity(model.shape(edge), by_curvature)?;
    Some(["CONVEX", "CONCAVE", "SMOOTH", "VARIABLE"][kind as usize])
}

/// The listed edges of a face (no seams).
fn face_edges(model: &Model, face: EntityRef) -> Vec<EntityRef> {
    let shapes = model.shape(face).sub_shapes(ShapeKind::Edge);
    let edges = &model.bodies[face.body].edges;
    (0..edges.len())
        .filter(|&i| shapes.iter().any(|s| s.is_same(&edges[i].shape)))
        .map(|index| EntityRef { body: face.body, kind: EntityType::Edge, index })
        .collect()
}

/// The faces of an edge's body that use it.
fn edge_faces(model: &Model, edge: EntityRef) -> Vec<EntityRef> {
    let shape = model.shape(edge);
    let faces = &model.bodies[edge.body].faces;
    (0..faces.len())
        .filter(|&i| faces[i].shape.sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(shape)))
        .map(|index| EntityRef { body: edge.body, kind: EntityType::Face, index })
        .collect()
}

/// Faces reached from `seeds` by crossing edges `cross` accepts into faces `enter` accepts.
fn flood(
    model: &Model,
    seeds: &[EntityRef],
    cross: impl Fn(EntityRef) -> bool,
    enter: impl Fn(EntityRef) -> bool,
) -> Vec<EntityRef> {
    let mut found: BTreeSet<EntityRef> = seeds.iter().copied().filter(|r| r.kind == EntityType::Face).collect();
    let mut queue: Vec<EntityRef> = found.iter().copied().collect();
    while let Some(face) = queue.pop() {
        for edge in face_edges(model, face) {
            if !cross(edge) {
                continue;
            }
            for other in edge_faces(model, edge) {
                if !found.contains(&other) && enter(other) {
                    found.insert(other);
                    queue.push(other);
                }
            }
        }
    }
    found.into_iter().collect()
}

/// Faces connected to the seeds across convex (or concave) edges.
pub fn connected_faces(model: &Model, seeds: &[EntityRef], wanted: &str) -> Vec<EntityRef> {
    flood(model, seeds, |e| convexity(model, e, true) == Some(wanted), |_| true)
}

/// The axis of a cylinder or cone.
fn axis_of(face: &Shape) -> Option<occt::Axis> {
    let s = face.surface_definition().ok()?;
    matches!(s.kind, 1 | 2).then_some(occt::Axis { origin: s.frame.origin, direction: s.frame.normal })
}

fn on_axis(axis: &occt::Axis, p: Vec3) -> bool {
    let rel = values::sub(p, axis.origin);
    let off = values::sub(rel, values::scale(axis.direction, values::dot(rel, axis.direction)));
    values::norm(off) < 1e-8
}

/// The faces of the hole a seed face is in: faces reached from it that are cylinders or cones on its axis, or
/// planes square to the axis bounded only by circles about it (counterbore floors).
pub fn hole_faces(model: &Model, seeds: &[EntityRef]) -> Vec<EntityRef> {
    let mut out = BTreeSet::new();
    for &seed in seeds.iter().filter(|r| r.kind == EntityType::Face) {
        let Some(axis) = axis_of(model.shape(seed)) else { continue };
        let in_hole = |face: EntityRef| -> bool {
            let shape = model.shape(face);
            if let Some(other) = axis_of(shape) {
                return values::norm(values::cross(other.direction, axis.direction)) < 1e-8
                    && on_axis(&axis, other.origin);
            }
            let Ok(plane) = shape.face_plane() else { return false };
            if values::norm(values::cross(plane.normal, axis.direction)) > 1e-8 {
                return false;
            }
            face_edges(model, face).iter().all(|e| {
                model.shape(*e).curve_definition().is_ok_and(|c| c.kind == 1 && on_axis(&axis, c.frame.origin))
            })
        };
        out.extend(flood(model, &[seed], |_| true, in_hole));
    }
    out.into_iter().collect()
}

/// A face's fillet radius, if it is a fillet: a cylinder, torus or sphere meeting at least two faces smoothly.
fn fillet_radius(model: &Model, face: EntityRef) -> Option<f64> {
    let shape = model.shape(face);
    let s = shape.surface_definition().ok()?;
    let radius = match s.kind {
        1 | 3 => s.radius1,
        4 => s.radius2,
        _ => return None,
    };
    let tangent = face_edges(model, face)
        .iter()
        .flat_map(|&e| edge_faces(model, e).into_iter().map(move |f| (e, f)))
        .filter(|&(e, f)| f != face && model.shape(e).faces_tangent_along(shape, model.shape(f), 1e-6))
        .map(|(_, f)| f)
        .collect::<BTreeSet<_>>();
    (tangent.len() >= 2).then_some(radius)
}

/// The fillets of the seeds' bodies whose radius compares as asked with the seeds'.
pub fn fillet_faces(model: &Model, seeds: &[EntityRef], compare: &str) -> Vec<EntityRef> {
    let mut out = BTreeSet::new();
    for &seed in seeds.iter().filter(|r| r.kind == EntityType::Face) {
        let Some(r0) = fillet_radius(model, seed) else { continue };
        for index in 0..model.bodies[seed.body].faces.len() {
            let face = EntityRef { body: seed.body, kind: EntityType::Face, index };
            let Some(r) = fillet_radius(model, face) else { continue };
            let tie = (r - r0).abs() < 1e-9;
            let keep = match compare {
                "LESS_EQUAL" => tie || r < r0,
                "GREATER_EQUAL" => tie || r > r0,
                _ => tie,
            };
            if keep {
                out.insert(face);
            }
        }
    }
    out.into_iter().collect()
}

/// Loops of edges: for faces, the edges of their outer boundaries that only one of them uses; for laminar edges
/// (a sheet's boundary), the laminar loop through each.
pub fn loop_edges(model: &Model, seeds: &[EntityRef]) -> Vec<EntityRef> {
    let mut out = BTreeSet::new();
    let faces: Vec<EntityRef> = seeds.iter().copied().filter(|r| r.kind == EntityType::Face).collect();
    for &face in &faces {
        let Ok(outer) = occt::outer_wire(model.shape(face)) else { continue };
        let outer = outer.sub_shapes(ShapeKind::Edge);
        for edge in face_edges(model, face) {
            let shared = edge_faces(model, edge).iter().filter(|f| faces.contains(f)).count() > 1;
            if !shared && outer.iter().any(|e| e.is_same(model.shape(edge))) {
                out.insert(edge);
            }
        }
    }
    let laminar = |e: EntityRef| edge_faces(model, e).len() == 1;
    let mut queue: Vec<EntityRef> =
        seeds.iter().copied().filter(|r| r.kind == EntityType::Edge && laminar(*r)).collect();
    out.extend(queue.iter().copied());
    while let Some(edge) = queue.pop() {
        let ends = model.shape(edge).sub_shapes(ShapeKind::Vertex);
        for index in 0..model.bodies[edge.body].edges.len() {
            let other = EntityRef { body: edge.body, kind: EntityType::Edge, index };
            if out.contains(&other) || !laminar(other) {
                continue;
            }
            if model.shape(other).sub_shapes(ShapeKind::Vertex).iter().any(|v| ends.iter().any(|w| w.is_same(v))) {
                out.insert(other);
                queue.push(other);
            }
        }
    }
    out.into_iter().collect()
}

/// A line edge's direction.
fn line_direction(model: &Model, edge: EntityRef) -> Option<Vec3> {
    let shape = model.shape(edge);
    (shape.curve_type().ok()? == occt::CurveType::Line).then(|| shape.axis().ok().map(|a| a.direction)).flatten()
}

fn parallel(a: Vec3, b: Vec3) -> bool {
    values::norm(values::cross(a, b)) < 1e-9
}

/// Line edges among `candidates` parallel (or anti-parallel) to any of `directions`.
pub fn parallel_edges(model: &Model, candidates: &[EntityRef], directions: &[Vec3]) -> Vec<EntityRef> {
    candidates
        .iter()
        .copied()
        .filter(|r| r.kind == EntityType::Edge)
        .filter(|r| line_direction(model, *r).is_some_and(|d| directions.iter().any(|&o| parallel(d, o))))
        .collect()
}

/// The line directions of some edges, and every edge of their bodies.
pub fn edge_directions(model: &Model, edges: &[EntityRef]) -> (Vec<Vec3>, Vec<EntityRef>) {
    let directions = edges.iter().filter_map(|r| line_direction(model, *r)).collect();
    let mut bodies: Vec<usize> = edges.iter().map(|r| r.body).collect();
    bodies.dedup();
    let all = bodies.into_iter().flat_map(|b| model.owned_by(b, Some(EntityType::Edge))).collect();
    (directions, all)
}

/// Faces on the side of an edge of a face, bounded by that face: everything reached from the edge's other face
/// without entering the bounding face.
pub fn loop_bounded_faces(model: &Model, found: &[EntityRef]) -> Vec<EntityRef> {
    let (Some(&face), Some(&edge)) =
        (found.iter().find(|r| r.kind == EntityType::Face), found.iter().find(|r| r.kind == EntityType::Edge))
    else {
        return Vec::new();
    };
    let start: Vec<EntityRef> = edge_faces(model, edge).into_iter().filter(|f| *f != face).collect();
    flood(model, &start, |_| true, |f| f != face)
}

/// Faces reached from a seed face without crossing the bounding edges or entering the bounding faces.
pub fn bounded_faces(model: &Model, found: &[EntityRef]) -> Vec<EntityRef> {
    let Some(&seed) = found.iter().find(|r| r.kind == EntityType::Face) else { return Vec::new() };
    let barrier_edges: Vec<EntityRef> = found.iter().copied().filter(|r| r.kind == EntityType::Edge).collect();
    let barrier_faces: Vec<EntityRef> =
        found.iter().copied().filter(|r| r.kind == EntityType::Face && *r != seed).collect();
    flood(model, &[seed], |e| !barrier_edges.contains(&e), |f| !barrier_faces.contains(&f))
}

/// What identifies an entity's size and shape: its geometry type, area or length, and its edges' lengths.
fn signature(model: &Model, r: EntityRef) -> Option<(String, f64, Vec<f64>)> {
    let shape = model.shape(r);
    match r.kind {
        EntityType::Face => {
            let kind = crate::query::geometry_type(model, r)?.to_string();
            let mut lengths: Vec<f64> =
                face_edges(model, r).iter().filter_map(|e| model.shape(*e).length().ok()).collect();
            lengths.sort_by(f64::total_cmp);
            Some((kind, shape.area().ok()?, lengths))
        }
        EntityType::Edge => {
            Some((crate::query::geometry_type(model, r)?.to_string(), shape.length().ok()?, Vec::new()))
        }
        _ => None,
    }
}

/// Faces or edges of the references' bodies with the same size and shape as a reference.
pub fn matching(model: &Model, references: &[EntityRef]) -> Vec<EntityRef> {
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1e-6);
    let mut out = BTreeSet::new();
    for &reference in references {
        let Some((kind, size, lengths)) = signature(model, reference) else { continue };
        for candidate in model.owned_by(reference.body, Some(reference.kind)) {
            let Some((k, s, l)) = signature(model, candidate) else { continue };
            if k == kind
                && close(s, size)
                && l.len() == lengths.len()
                && l.iter().zip(&lengths).all(|(a, b)| close(*a, *b))
            {
                out.insert(candidate);
            }
        }
    }
    out.into_iter().collect()
}

/// Planar faces whose normal is `normal` (or its opposite, if allowed).
pub fn planes_with_normal(model: &Model, found: &[EntityRef], normal: Vec3, antiparallel: bool) -> Vec<EntityRef> {
    found
        .iter()
        .copied()
        .filter(|r| r.kind == EntityType::Face && crate::query::geometry_type(model, *r) == Some("PLANE"))
        .filter(|r| {
            model.shape(*r).face_plane().is_ok_and(|p| {
                let d = values::dot(p.normal, normal);
                d > 1.0 - 1e-9 || (antiparallel && d < -1.0 + 1e-9)
            })
        })
        .collect()
}

/// Entities wholly in front of (or on) a plane.
pub fn in_front_of(model: &Model, found: &[EntityRef], plane: occt::Frame) -> R<Vec<EntityRef>> {
    Ok(found
        .iter()
        .copied()
        .filter(|r| {
            let shape = if r.kind == EntityType::Body { &model.bodies[r.body].shape } else { model.shape(*r) };
            shape.bounding_box_in(plane, true).is_ok_and(|b| b.min.z >= -1e-8)
        })
        .collect())
}
