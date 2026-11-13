//! Topological and geometric queries over a body's faces, edges and vertices: adjacency, tangent chains, ends of
//! edges, and picking entities by size or position.

use crate::model::{BodyType, EntityRef, EntityType, Model};
use crate::values;
use occt::{Shape, ShapeKind, Vec3};
use std::collections::BTreeSet;

/// Two entities are tangent if they meet within this angle (radians).
pub const TANGENT_TOLERANCE: f64 = 1e-6;

/// Entities tied within this distance (metres) count as equal when picking the closest or farthest.
const TIE: f64 = 1e-8;

fn shape_kind(kind: EntityType) -> ShapeKind {
    match kind {
        EntityType::Body => ShapeKind::Compound,
        EntityType::Face => ShapeKind::Face,
        EntityType::Edge => ShapeKind::Edge,
        EntityType::Vertex => ShapeKind::Vertex,
    }
}

/// The edges or vertices of an entity that the body lists (so no seams or hidden vertices); a vertex or edge is
/// its own.
fn connectors(model: &Model, r: EntityRef, via: EntityType) -> Vec<Shape> {
    let shape = model.shape(r);
    let listed = model.body(r).entities(via);
    if r.kind == via {
        return vec![shape.clone()];
    }
    shape.sub_shapes(shape_kind(via)).into_iter().filter(|s| listed.iter().any(|e| e.shape.is_same(s))).collect()
}

/// Entities of `kind` that share a vertex (`via` vertex) or an edge (`via` edge) with any of `seeds`, other than
/// the seeds themselves.
pub fn adjacent(model: &Model, seeds: &[EntityRef], via: EntityType, kind: Option<EntityType>) -> Vec<EntityRef> {
    let seed_set: BTreeSet<EntityRef> = seeds.iter().copied().collect();
    let mut out = BTreeSet::new();
    for seed in seeds {
        if seed.kind == EntityType::Body {
            continue;
        }
        let shared = connectors(model, *seed, via);
        for k in [EntityType::Face, EntityType::Edge, EntityType::Vertex] {
            if kind.is_some_and(|kind| kind != k) {
                continue;
            }
            for (index, entity) in model.bodies[seed.body].entities(k).iter().enumerate() {
                let candidate = EntityRef { body: seed.body, kind: k, index };
                if seed_set.contains(&candidate) {
                    continue;
                }
                let touches = if k == via {
                    shared.iter().any(|s| s.is_same(&entity.shape))
                } else {
                    entity.shape.sub_shapes(shape_kind(via)).iter().any(|s| shared.iter().any(|t| t.is_same(s)))
                };
                if touches {
                    out.insert(candidate);
                }
            }
        }
    }
    out.into_iter().collect()
}

/// The start or end vertex of each edge, in the edge's own direction. Closed edges have none.
/// An edge's end vertices, its start first, as Onshape's edge runs (`Info::sense`, else as its shape runs).
pub fn end_vertices(model: &Model, r: EntityRef) -> Vec<occt::Shape> {
    let edge = crate::ops::onshape_edge(model, r);
    // An edge OCCT split at a seam ends where its last piece does: the body's vertices at the whole edge's ends (an
    // arc a split left across a cylinder's seam, which a frame chains to the lines at its ends, batch 138 c005).
    let whole = crate::ops::whole_edge(model, r);
    if !whole.is_same(&edge)
        && let (Ok(a), Ok(b)) = (whole.edge_tangent(0.0, false), whole.edge_tangent(1.0, false))
    {
        let at = |p: occt::Vec3| {
            model.bodies[r.body].vertices.iter().map(|v| &v.shape).find(|v| {
                v.vertex_point().is_ok_and(|q| crate::values::norm(crate::values::sub(q, p)) < 1e-7)
            })
        };
        if let (Some(first), Some(last)) = (at(a.origin), at(b.origin)) {
            return vec![first.clone(), last.clone()];
        }
    }
    let mut ends = edge.edge_end_vertices().unwrap_or_default();
    if let (Ok(start), [first, last]) = (edge.edge_tangent(0.0, false), &ends[..]) {
        let gap = |v: &occt::Shape| {
            v.vertex_point().map_or(f64::MAX, |p| crate::values::norm(crate::values::sub(p, start.origin)))
        };
        if gap(last) < gap(first) {
            ends.reverse();
        }
    }
    ends
}

pub fn edge_vertices(model: &Model, edges: &[EntityRef], at_start: bool) -> Vec<EntityRef> {
    let mut out = Vec::new();
    for r in edges.iter().filter(|r| r.kind == EntityType::Edge) {
        let ends = end_vertices(model, *r);
        let Some(v) = (if at_start { ends.first() } else { ends.last() }) else { continue };
        let vertices = &model.bodies[r.body].vertices;
        if let Some(index) = vertices.iter().position(|e| e.shape.is_same(v)) {
            out.push(EntityRef { body: r.body, kind: EntityType::Vertex, index });
        }
    }
    out
}

/// Edges connected to the seeds through vertices where they continue smoothly, flood-filling.
pub fn tangent_connected_edges(model: &Model, seeds: &[EntityRef]) -> Vec<EntityRef> {
    let mut found: BTreeSet<EntityRef> = seeds.iter().copied().filter(|r| r.kind == EntityType::Edge).collect();
    let mut queue: Vec<EntityRef> = found.iter().copied().collect();
    while let Some(edge) = queue.pop() {
        let shape = model.shape(edge).clone();
        for vertex in connectors(model, edge, EntityType::Vertex) {
            for (index, other) in model.bodies[edge.body].edges.iter().enumerate() {
                let candidate = EntityRef { body: edge.body, kind: EntityType::Edge, index };
                if found.contains(&candidate) {
                    continue;
                }
                let meets = other.shape.sub_shapes(ShapeKind::Vertex).iter().any(|v| v.is_same(&vertex));
                if meets && shape.tangent_at(&other.shape, &vertex, TANGENT_TOLERANCE) {
                    found.insert(candidate);
                    queue.push(candidate);
                }
            }
        }
    }
    found.into_iter().collect()
}

/// Faces connected to the seeds through edges along which they meet smoothly, flood-filling.
pub fn tangent_connected_faces(model: &Model, seeds: &[EntityRef], tolerance: f64) -> Vec<EntityRef> {
    let mut found: BTreeSet<EntityRef> = seeds.iter().copied().filter(|r| r.kind == EntityType::Face).collect();
    let mut queue: Vec<EntityRef> = found.iter().copied().collect();
    while let Some(face) = queue.pop() {
        let shape = model.shape(face).clone();
        for edge in connectors(model, face, EntityType::Edge) {
            for (index, other) in model.bodies[face.body].faces.iter().enumerate() {
                let candidate = EntityRef { body: face.body, kind: EntityType::Face, index };
                if found.contains(&candidate) {
                    continue;
                }
                let shares = other.shape.sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(&edge));
                if shares && edge.faces_tangent_along(&shape, &other.shape, tolerance) {
                    found.insert(candidate);
                    queue.push(candidate);
                }
            }
        }
    }
    found.into_iter().collect()
}

/// The entities closest to a point, with ties.
pub fn closest_to(model: &Model, found: &[EntityRef], point: Vec3) -> Vec<EntityRef> {
    let Ok(vertex) = occt::vertex(point) else { return Vec::new() };
    let distances: Vec<f64> =
        found.iter().map(|r| model.shape(*r).distance(&vertex).map_or(f64::INFINITY, |d| d.distance)).collect();
    pick(found, &distances, false)
}

/// The entities reaching farthest along a direction, with ties.
pub fn farthest_along(model: &Model, found: &[EntityRef], direction: Vec3) -> Vec<EntityRef> {
    let Some(direction) = values::normalize(direction) else { return Vec::new() };
    let reach: Vec<f64> = found
        .iter()
        .map(|r| {
            let frame =
                occt::Frame { origin: Vec3::new(0.0, 0.0, 0.0), normal: direction, x: values::canonical_x(direction) };
            model.shape(*r).bounding_box_in(frame, true).map_or(f64::NEG_INFINITY, |b| b.max.z)
        })
        .collect();
    pick(found, &reach, true)
}

fn pick(found: &[EntityRef], scores: &[f64], largest: bool) -> Vec<EntityRef> {
    let best = scores.iter().copied().fold(if largest { f64::NEG_INFINITY } else { f64::INFINITY }, |a, b| {
        if largest { a.max(b) } else { a.min(b) }
    });
    if !best.is_finite() {
        return Vec::new();
    }
    found.iter().zip(scores).filter(|(_, s)| (*s - best).abs() <= TIE).map(|(r, _)| *r).collect()
}

/// The largest or smallest entities by length, area or volume, among those of the highest dimension, with ties
/// (relative 1e-9).
pub fn by_size(model: &Model, found: &[EntityRef], largest: bool) -> Vec<EntityRef> {
    let dimension = |r: &EntityRef| match r.kind {
        EntityType::Body => match model.body(*r).body_type {
            BodyType::Solid => 3,
            BodyType::Sheet => 2,
            BodyType::Wire => 1,
            _ => 0,
        },
        EntityType::Face => 2,
        EntityType::Edge => 1,
        EntityType::Vertex => 0,
    };
    let Some(top) = found.iter().map(dimension).max() else { return Vec::new() };
    let candidates: Vec<EntityRef> = found.iter().copied().filter(|r| dimension(r) == top).collect();
    let sizes: Vec<f64> = candidates
        .iter()
        .map(|r| {
            let shape = model.shape(*r);
            match top {
                3 => shape.volume(),
                2 => shape.area(),
                1 => shape.length(),
                _ => Ok(0.0),
            }
            .unwrap_or(0.0)
        })
        .collect();
    let best = sizes.iter().copied().fold(if largest { f64::NEG_INFINITY } else { f64::INFINITY }, |a, b| {
        if largest { a.max(b) } else { a.min(b) }
    });
    candidates
        .into_iter()
        .zip(sizes)
        .filter(|(_, s)| (s - best).abs() <= 1e-9 * best.abs().max(1e-30))
        .map(|(r, _)| r)
        .collect()
}

/// Which side of an edge of its boundary a face lies on, seen from the face's normal (a sketch region's, the
/// sketch's): `Some(true)` to the left of the edge's curve's own direction, `Some(false)` to the right, `None` if
/// the edge doesn't bound the face.
pub fn face_side(model: &Model, face: EntityRef, edge: EntityRef) -> Option<bool> {
    let (f, e) = (model.shape(face), model.shape(edge));
    if !f.sub_shapes(ShapeKind::Edge).iter().any(|s| s.is_same(e)) {
        return None;
    }
    let at = e.edge_tangent(0.5, false).ok()?;
    let tangent = if e.is_reversed() { values::scale(at.direction, -1.0) } else { at.direction };
    let normal = match model.body(face).sketch_plane {
        Some(plane) => plane.normal,
        None => f.face_tangent_plane(0.5, 0.5).ok()?.normal,
    };
    let left = values::normalize(values::cross(normal, tangent))?;
    let on_face = |offset: f64| {
        occt::vertex(values::add(at.origin, values::scale(left, offset)))
            .and_then(|v| v.distance(f))
            .is_ok_and(|d| d.distance < 1e-9)
    };
    const STEP: f64 = 1e-6;
    if on_face(STEP) {
        Some(true)
    } else if on_face(-STEP) {
        Some(false)
    } else {
        None
    }
}
