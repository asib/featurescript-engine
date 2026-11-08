//! Sheet metal (`@updateSheetMetalGeometry`). std's sheet metal features keep a model as a definition sheet
//! body: walls are its faces (with `SMAttribute`s of type `WALL`), joints its edges (`JOINT`: bends with an inner
//! radius, rips), corners its vertices, and every entity has its own `SMAssociationAttribute`. The model's
//! attribute says how much material lies in front of the sheet (along its normals) and behind it. This builds
//! the folded part from that: each bend is the sheet's edge rounded on the inside so that the part's inner
//! surface has the bend's radius, then the sheet is thickened front and back. The part's faces and edges take
//! the association attributes of what they came from, which is how features find the definition entities of
//! what's picked on the part. (Fixture batch 012.)

use std::rc::Rc;

use crate::model::{Attributes, Body, BodyType, EntityRef, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, add, cross, dot, field, norm, normalize, scale, sub};
use crate::{occt_error, query, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Shape, ShapeKind, Vec3};

const TOLERANCE: f64 = 1e-7;

fn is_type(value: &Value, type_name: &str) -> bool {
    value.tag.is_some_and(|t| &*fs_interp::types::name(t) == type_name)
}

/// The `SMAttribute` of the given object type among some attributes.
fn sm_attribute<'a>(attributes: &'a Attributes, object_type: &str) -> Option<&'a Value> {
    attributes
        .unnamed
        .iter()
        .find(|a| is_type(a, "SMAttribute") && a.get("objectType").and_then(Value::as_str) == Some(object_type))
}

/// The `SMAssociationAttribute` among some attributes.
fn association(attributes: &Attributes) -> Option<&Value> {
    attributes.unnamed.iter().find(|a| is_type(a, "SMAssociationAttribute"))
}

/// The value of an attribute field kept as `{ "value" : ... }`.
fn attribute_value<'a>(attribute: &'a Value, key: &str) -> Option<&'a Value> {
    attribute.get(key).and_then(|v| v.get("value"))
}

/// Whether a body is a sheet metal part: one carrying an association attribute, which only sheet metal gives.
pub fn is_sheet_metal_body(body: &Body) -> bool {
    body.is_sheet_metal_definition() || association(&body.attributes).is_some()
}

/// A bend of the definition sheet: its edge, the inner radius, the radius at the sheet (the inner radius plus
/// the thickness on the inside), and the axis of the rounded sheet.
struct Bend {
    edge: Shape,
    sheet_radius: f64,
    axis_origin: Vec3,
    axis_direction: Vec3,
    /// A point of the edge (where the axis was found), from which positions along the axis are measured.
    at: Vec3,
    /// How far each wall is cut back from the edge to where the bend leaves it.
    setback: f64,
    /// The part's inner and outer radii.
    radii: (f64, f64),
    /// From the axis towards where the bend meets each wall.
    toward: (Vec3, Vec3),
}

fn normal_at(face: &Shape, p: Vec3) -> R<Vec3> {
    let (fu, fv) = face.face_fraction(p).map_err(occt_error)?;
    Ok(face.face_tangent_plane(fu, fv).map_err(occt_error)?.normal)
}

/// The direction from a point on an edge of a face into the face, square to the edge.
fn into_face(face: &Shape, m: Vec3, along: Vec3) -> R<Vec3> {
    let n = normal_at(face, m)?;
    let d = normalize(cross(n, along)).ok_or_else(|| regen_error("SHEET_METAL_REBUILD_ERROR"))?;
    let probe = add(m, scale(d, 1e-6));
    let inside = occt::vertex(probe).and_then(|v| v.distance(face)).map_err(occt_error)?.distance < 1e-9;
    Ok(if inside { d } else { scale(d, -1.0) })
}

fn bend_of(def: &Body, edge_index: usize, radius: f64, front: f64, back: f64) -> R<Bend> {
    let thickness = front + back;
    let edge = def.edges[edge_index].shape.clone();
    let faces: Vec<&Shape> = def
        .faces
        .iter()
        .map(|f| &f.shape)
        .filter(|f| f.sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(&edge)))
        .collect();
    let [f1, f2] = faces[..] else { return Err(error("@updateSheetMetalGeometry: a bend needs two walls")) };
    let tangent = edge.edge_tangent(0.5, true).map_err(occt_error)?;
    let (m, along) = (tangent.origin, tangent.direction);
    let (d1, d2) = (into_face(f1, m, along)?, into_face(f2, m, along)?);
    let n1 = normal_at(f1, m)?;
    // The inside of the bend is between the walls; the sheet is rounded there, at the inner radius plus however
    // much material lies on the inside.
    let inside = if dot(n1, d2) > 0.0 { front } else { back };
    let sheet_radius = radius + inside;
    let alpha = dot(d1, d2).clamp(-1.0, 1.0).acos();
    let bisector = normalize(add(d1, d2)).ok_or_else(|| regen_error("SHEET_METAL_NO_0_ANGLE_BEND"))?;
    let axis_origin = add(m, scale(bisector, sheet_radius / (alpha / 2.0).sin()));
    let setback = sheet_radius / (alpha / 2.0).tan();
    let leaves = |d: Vec3| normalize(sub(add(m, scale(d, setback)), axis_origin));
    let toward = (
        leaves(d1).ok_or_else(|| regen_error("SHEET_METAL_REBUILD_ERROR"))?,
        leaves(d2).ok_or_else(|| regen_error("SHEET_METAL_REBUILD_ERROR"))?,
    );
    let radii = (sheet_radius - inside, sheet_radius - inside + thickness);
    Ok(Bend { edge, sheet_radius, axis_origin, axis_direction: along, at: m, setback, radii, toward })
}

fn distance_to_segment(p: Vec3, a: Vec3, b: Vec3) -> f64 {
    let ab = sub(b, a);
    let t = (dot(sub(p, a), ab) / dot(ab, ab).max(1e-300)).clamp(0.0, 1.0);
    norm(sub(p, add(a, scale(ab, t))))
}

fn segment(edge: &Shape) -> Option<(Vec3, Vec3)> {
    if edge.curve_type().ok()? != occt::CurveType::Line {
        return None;
    }
    Some((edge.edge_tangent(0.0, false).ok()?.origin, edge.edge_tangent(1.0, false).ok()?.origin))
}

/// What the part's entities came from in the definition: the definition entity whose association attribute each
/// takes. Walls give the faces offset from them; bends the cylinders round them and the lines where those meet
/// the walls; edges on the sheet's boundary the faces across the thickness there, and the part's edges along
/// them; vertices the part's edges across the thickness at them.
fn associate(def: &Body, bends: &[Bend], thickness: f64, kind: EntityType, shape: &Shape) -> Option<EntityRef> {
    let reach = thickness + TOLERANCE;
    let def_ref = |kind, index| Some(EntityRef { body: usize::MAX, kind, index });
    let boundary: Vec<usize> = (0..def.edges.len())
        .filter(|&i| {
            let e = &def.edges[i].shape;
            def.faces.iter().filter(|f| f.shape.sub_shapes(ShapeKind::Edge).iter().any(|x| x.is_same(e))).count() == 1
        })
        .collect();
    match kind {
        EntityType::Face => {
            let p = shape.face_inner_point().ok()?;
            let n = normal_at(shape, p).ok()?;
            if shape.surface_type().ok()? == occt::SurfaceType::Plane {
                for (i, wall) in def.faces.iter().enumerate() {
                    let plane = wall.shape.face_plane().ok()?;
                    let off = dot(sub(p, plane.origin), plane.normal);
                    if dot(n, plane.normal).abs() > 1.0 - 1e-9 && off.abs() <= reach {
                        let foot = sub(p, scale(plane.normal, off));
                        if occt::vertex(foot).and_then(|v| v.distance(&wall.shape)).ok()?.distance < 1e-6 {
                            return def_ref(EntityType::Face, i);
                        }
                    }
                }
                // Across the thickness at a boundary edge: the face's plane holds the edge.
                let mut best: Option<(f64, usize)> = None;
                for &i in &boundary {
                    let Some((a, b)) = segment(&def.edges[i].shape) else { continue };
                    let in_plane = dot(sub(a, p), n).abs() < 1e-6 && dot(sub(b, p), n).abs() < 1e-6;
                    let d = distance_to_segment(p, a, b);
                    if in_plane && d <= reach && best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, i));
                    }
                }
                return best.and_then(|(_, i)| def_ref(EntityType::Edge, def_edge_index(def, &def.edges[i].shape)?));
            }
            let s = shape.surface_definition().ok()?;
            if s.kind == 1 {
                for bend in bends {
                    let parallel = norm(cross(s.frame.normal, bend.axis_direction)) < 1e-9;
                    let offset = sub(s.frame.origin, bend.axis_origin);
                    let apart = norm(sub(offset, scale(bend.axis_direction, dot(offset, bend.axis_direction))));
                    if parallel && apart < 1e-6 {
                        return def_ref(EntityType::Edge, def_edge_index(def, &bend.edge)?);
                    }
                }
            }
            None
        }
        EntityType::Edge => {
            let (a, b) = segment(shape)?;
            let (m, d) = (scale(add(a, b), 0.5), normalize(sub(b, a))?);
            // Along a boundary edge, or where a bend meets a wall.
            let mut best: Option<(f64, usize)> = None;
            for (i, e) in def.edges.iter().enumerate() {
                let Some((ea, eb)) = segment(&e.shape) else { continue };
                let Some(ed) = normalize(sub(eb, ea)) else { continue };
                let limit = match bends.iter().find(|bend| bend.edge.is_same(&e.shape)) {
                    Some(bend) => {
                        norm(sub(bend.axis_origin, ea_foot(ea, ed, bend.axis_origin))) + bend.sheet_radius + reach
                    }
                    None if boundary.contains(&i) => reach,
                    None => continue,
                };
                let dist = distance_to_segment(m, ea, eb);
                if dot(d, ed).abs() > 1.0 - 1e-9 && dist <= limit && best.is_none_or(|(bd, _)| dist < bd) {
                    best = Some((dist, i));
                }
            }
            if let Some((_, i)) = best {
                return def_ref(EntityType::Edge, i);
            }
            // Across the thickness at a vertex.
            if norm(sub(b, a)) <= reach {
                for (i, v) in def.vertices.iter().enumerate() {
                    let q = v.shape.vertex_point().ok()?;
                    if distance_to_segment(q, a, b) <= TOLERANCE || norm(sub(m, q)) <= reach {
                        return def_ref(EntityType::Vertex, i);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// The foot of the perpendicular from `p` to the line through `a` along unit `d`.
fn ea_foot(a: Vec3, d: Vec3, p: Vec3) -> Vec3 {
    add(a, scale(d, dot(sub(p, a), d)))
}

fn def_edge_index(def: &Body, edge: &Shape) -> Option<usize> {
    def.edges.iter().position(|e| e.shape.is_same(edge))
}

/// Builds the folded part of one model, replacing the part it had.
fn regenerate(model: &mut Model, id: &FeatureId, def_index: usize) -> R<()> {
    let def = model.bodies[def_index].clone();
    let model_attribute =
        sm_attribute(&def.attributes, "MODEL").ok_or_else(|| regen_error("SHEET_METAL_REBUILD_ERROR"))?.clone();
    let thickness_of = |key: &str| -> R<f64> {
        match attribute_value(&model_attribute, key) {
            Some(v) => values::length(v, "sheet metal thickness"),
            None => Ok(0.0),
        }
    };
    let (front, back) = (thickness_of("frontThickness")?, thickness_of("backThickness")?);
    let thickness = front + back;
    for face in &def.faces {
        if face.shape.surface_type().map_err(occt_error)? != occt::SurfaceType::Plane {
            return Err(error("@updateSheetMetalGeometry: rolled walls aren't supported yet"));
        }
    }
    let mut bends = Vec::new();
    let mut rips = Vec::new();
    for (i, edge) in def.edges.iter().enumerate() {
        let Some(joint) = sm_attribute(&edge.attributes, "JOINT") else { continue };
        match attribute_value(joint, "jointType").and_then(Value::as_str) {
            Some("BEND") => {
                let radius = values::length(
                    attribute_value(joint, "radius").ok_or_else(|| regen_error("SHEET_METAL_REBUILD_ERROR"))?,
                    "bend radius",
                )?;
                bends.push(bend_of(&def, i, radius, front, back)?);
            }
            Some("RIP") => {
                let style = attribute_value(joint, "jointStyle").and_then(Value::as_str).unwrap_or("EDGE");
                if style != "EDGE" {
                    return Err(error(format!("@updateSheetMetalGeometry: {style} rips aren't supported yet")));
                }
                rips.push(i);
            }
            Some("TANGENT") | None => {}
            Some(other) => {
                return Err(error(format!("@updateSheetMetalGeometry: {other} joints aren't supported yet")));
            }
        }
    }
    let clearance = match attribute_value(&model_attribute, "minimalClearance") {
        Some(v) => values::length(v, "minimal clearance")?,
        None => 2e-5,
    };
    let solid = build_part(&def, &bends, &rips, front, back, clearance)?;

    // Replace the model's part, keeping its identity, or make one.
    let def_association = association(&def.attributes).cloned();
    let old = model.bodies.iter().position(|b| {
        b.body_type == BodyType::Solid
            && def_association.is_some()
            && association(&b.attributes) == def_association.as_ref()
    });
    let mut attributes = Attributes::default();
    if let Some(a) = &def_association {
        attributes.unnamed.push(a.clone());
    }
    let new = match old {
        Some(o) => {
            let old_body = model.bodies[o].clone();
            model.remove_bodies(&[o]);
            NewBody { attributes: Rc::new(attributes), ..NewBody::replacing(&old_body, solid) }
        }
        None => {
            NewBody { attributes: Rc::new(attributes), ..NewBody::new(solid, BodyType::Solid, Info::new(id.clone())) }
        }
    };
    let info = Info::new(id.clone());
    let index = model.add_body(new, &Origins::default(), &info, &|_| None);
    // The definition may have moved in the list.
    let def = model
        .bodies
        .iter()
        .find(|b| b.tid == def.tid)
        .cloned()
        .ok_or_else(|| regen_error("SHEET_METAL_REBUILD_ERROR"))?;
    for kind in [EntityType::Face, EntityType::Edge] {
        for i in 0..model.bodies[index].entities(kind).len() {
            let shape = model.bodies[index].entities(kind)[i].shape.clone();
            let Some(from) = associate(&def, &bends, thickness, kind, &shape) else { continue };
            let Some(a) = association(&def.entities(from.kind)[from.index].attributes).cloned() else { continue };
            model.update_attributes(EntityRef { body: index, kind, index: i }, |attributes| {
                attributes.unnamed.retain(|v| !is_type(v, "SMAssociationAttribute"));
                attributes.unnamed.push(a);
            });
        }
    }
    Ok(())
}

/// How far a wall is cut back at a rip (edge-to-edge style): to clear the other wall's material, and the minimal
/// clearance more. Two walls meeting at a right angle with their material outside both stop that clearance short
/// of the corner, leaving it open (batch 012, c007).
fn rip_setback(wall: &Shape, other: &Shape, edge: &Shape, front: f64, back: f64, clearance: f64) -> R<f64> {
    let tangent = edge.edge_tangent(0.5, true).map_err(occt_error)?;
    let (m, along) = (tangent.origin, tangent.direction);
    let into = into_face(wall, m, along)?;
    let n = normal_at(other, m)?;
    // The other wall's material lies between its definition offset by -back and +front along its normal; along
    // `into`, that's between these distances from the edge.
    let c = dot(into, n);
    if c.abs() < 1e-12 {
        return Ok(clearance);
    }
    let reach = (front / c).max(-back / c).max(0.0);
    Ok(reach + clearance)
}

/// The folded part, piece by piece: each wall cut back where it meets bends and rips and thickened, and each bend
/// a ring sector along the stretch both its walls still reach; the pieces fused. Where bends meet at a corner,
/// each stops where the other cuts its walls back, leaving the corner open (Onshape's simple corner).
fn build_part(def: &Body, bends: &[Bend], rips: &[usize], front: f64, back: f64, clearance: f64) -> R<Shape> {
    let faces_of_edge = |edge: &Shape| -> Vec<&Shape> {
        def.faces
            .iter()
            .map(|f| &f.shape)
            .filter(|f| f.sub_shapes(ShapeKind::Edge).iter().any(|e| e.is_same(edge)))
            .collect()
    };
    let mut pieces: Vec<Shape> = Vec::new();
    let mut cut_walls: Vec<Shape> = Vec::new();
    for wall in &def.faces {
        let mut edges = Vec::new();
        let mut distances = Vec::new();
        for edge in wall.shape.sub_shapes(ShapeKind::Edge) {
            if let Some(bend) = bends.iter().find(|b| b.edge.is_same(&edge)) {
                edges.push(edge);
                distances.push(-bend.setback);
            } else if rips.iter().any(|&r| def.edges[r].shape.is_same(&edge)) {
                let other = faces_of_edge(&edge).into_iter().find(|f| !f.is_same(&wall.shape));
                let Some(other) = other else { continue };
                let setback = rip_setback(&wall.shape, other, &edge, front, back, clearance)?;
                edges.push(edge);
                distances.push(-setback);
            }
        }
        let cut = if edges.is_empty() {
            wall.shape.clone()
        } else {
            occt::extend_planar_face(&wall.shape, &edges, &distances)
                .map_err(|_| regen_error("SHEET_METAL_REBUILD_ERROR"))?
        };
        let solid = occt::thicken(&cut, front, back).map_err(|_| regen_error("SHEET_METAL_CANNOT_THICKEN"))?.shape();
        pieces.push(solid);
        cut_walls.push(cut);
    }
    for bend in bends {
        // Where the cut-back walls' edges along the bend run: the bend covers the stretch both reach.
        let mut span = (f64::MIN, f64::MAX);
        let walls = faces_of_edge(&bend.edge);
        for wall in &walls {
            let index = def.faces.iter().position(|f| f.shape.is_same(wall)).expect("a wall of the model");
            let line_point = |toward: Vec3| add(bend.axis_origin, scale(toward, bend.sheet_radius));
            let mut reach: Option<(f64, f64)> = None;
            for edge in cut_walls[index].sub_shapes(ShapeKind::Edge) {
                let Some((a, b)) = segment(&edge) else { continue };
                for toward in [bend.toward.0, bend.toward.1] {
                    let p = line_point(toward);
                    let off = |q: Vec3| {
                        let d = sub(q, p);
                        norm(sub(d, scale(bend.axis_direction, dot(d, bend.axis_direction))))
                    };
                    if off(a) < 1e-7 && off(b) < 1e-7 {
                        let (ta, tb) =
                            (dot(sub(a, bend.at), bend.axis_direction), dot(sub(b, bend.at), bend.axis_direction));
                        reach = Some((ta.min(tb), ta.max(tb)));
                    }
                }
            }
            let (lo, hi) = reach.ok_or_else(|| regen_error("SHEET_METAL_REBUILD_ERROR"))?;
            span = (span.0.max(lo), span.1.min(hi));
        }
        if span.1 - span.0 <= 1e-9 {
            continue;
        }
        let centre = add(bend.axis_origin, scale(bend.axis_direction, span.0));
        let (r_in, r_out) = bend.radii;
        let (u1, u2) = bend.toward;
        let um = normalize(add(u1, u2)).ok_or_else(|| regen_error("SHEET_METAL_REBUILD_ERROR"))?;
        let at = |u: Vec3, r: f64| add(centre, scale(u, r));
        let edges = [
            occt::arc_through(at(u1, r_in), at(um, r_in), at(u2, r_in)).map_err(occt_error)?,
            occt::line_edge(at(u2, r_in), at(u2, r_out)).map_err(occt_error)?,
            occt::arc_through(at(u2, r_out), at(um, r_out), at(u1, r_out)).map_err(occt_error)?,
            occt::line_edge(at(u1, r_out), at(u1, r_in)).map_err(occt_error)?,
        ];
        let section = occt::fill_edges(&edges).map_err(occt_error)?;
        let piece = occt::prism(&section, scale(bend.axis_direction, span.1 - span.0)).map_err(occt_error)?.shape();
        pieces.push(piece);
    }
    let first = pieces.remove(0);
    let solid = if pieces.is_empty() {
        first
    } else {
        occt::boolean(occt::BooleanKind::Union, &[first], &pieces, false)
            .map_err(|_| regen_error("SHEET_METAL_REBUILD_ERROR"))?
            .shape()
    };
    let solids = solid.sub_shapes(ShapeKind::Solid);
    Ok(if solids.len() == 1 { solids.into_iter().next().expect("one solid") } else { solid })
}

/// `@updateSheetMetalGeometry`: rebuilds the parts of the models whose definition entities changed.
pub fn update(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let mut defs: Vec<u64> = Vec::new();
    for r in query::evaluate(model, field(definition, "entities"))? {
        let body = &model.bodies[r.body];
        if body.is_sheet_metal_definition() && !defs.contains(&body.tid) {
            defs.push(body.tid);
        }
    }
    for tid in defs {
        let Some(index) = model.bodies.iter().position(|b| b.tid == tid) else { continue };
        regenerate(model, &id, index)?;
    }
    Ok(Value::UNDEFINED)
}

/// Sample points of a face, edge or vertex.
fn samples(s: &Shape) -> Vec<Vec3> {
    match s.kind() {
        Some(ShapeKind::Vertex) => s.vertex_point().into_iter().collect(),
        Some(ShapeKind::Edge) => {
            [0.0, 0.5, 1.0].iter().filter_map(|&t| s.edge_tangent(t, false).ok().map(|a| a.origin)).collect()
        }
        _ => s.face_inner_point().into_iter().collect(),
    }
}

fn lies_on(points: &[Vec3], target: &Shape) -> bool {
    !points.is_empty()
        && points.iter().all(|p| occt::vertex(*p).and_then(|v| v.distance(target)).is_ok_and(|d| d.distance < 1e-9))
}

/// `@opEdgeChange`: free straight edges of planar faces moved within their faces' planes by an offset (outward
/// for a positive one), the neighbouring edges running on to meet them. The faces, edges and vertices keep their
/// identities. Transforms and moving edges onto other faces aren't supported yet.
pub fn edge_change(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let options = field(definition, "edgeChangeOptions")
        .as_array()
        .ok_or_else(|| error("@opEdgeChange: edgeChangeOptions must be an array"))?
        .clone();
    // (body, face, edge, offset) for each option.
    let mut changes: Vec<(usize, Shape, Shape, f64)> = Vec::new();
    for option in options.iter() {
        for key in ["transformList", "replaceFace"] {
            let v = field(option, key);
            if !v.is_undefined() && v.as_array().is_none_or(|a| !a.is_empty()) {
                return Err(error(format!("@opEdgeChange: {key} isn't supported yet")));
            }
        }
        let edge = query::evaluate(model, field(option, "edge"))?
            .into_iter()
            .find(|r| r.kind == EntityType::Edge)
            .ok_or_else(|| regen_error("CANNOT_RESOLVE_ENTITIES"))?;
        let face = query::evaluate(model, field(option, "face"))?
            .into_iter()
            .find(|r| r.kind == EntityType::Face)
            .ok_or_else(|| regen_error("CANNOT_RESOLVE_ENTITIES"))?;
        let offset = match field(option, "offset") {
            v if v.is_undefined() => 0.0,
            v => values::length(v, "opEdgeChange offset")?,
        };
        changes.push((edge.body, model.shape(face).clone(), model.shape(edge).clone(), offset));
    }
    let mut bodies: Vec<usize> = changes.iter().map(|c| c.0).collect();
    bodies.sort_unstable();
    bodies.dedup();
    let mut replacements = Vec::new();
    for &b in &bodies {
        let old = model.bodies[b].clone();
        let mut faces: Vec<Shape> = old.faces.iter().map(|f| f.shape.clone()).collect();
        let mut grown: Vec<(usize, Shape)> = Vec::new();
        for (i, face) in old.faces.iter().enumerate() {
            let mine: Vec<&(usize, Shape, Shape, f64)> =
                changes.iter().filter(|c| c.0 == b && c.1.is_same(&face.shape)).collect();
            if mine.is_empty() {
                continue;
            }
            let edges: Vec<Shape> = mine.iter().map(|c| c.2.clone()).collect();
            let distances: Vec<f64> = mine.iter().map(|c| c.3).collect();
            let new_face = occt::extend_planar_face(&face.shape, &edges, &distances)
                .map_err(|_| regen_error("EDGE_CHANGE_FAILED"))?;
            faces[i] = new_face.clone();
            grown.push((i, new_face));
        }
        let shell = occt::sew_faces(&faces, 1e-7).map_err(occt_error)?;
        let mut origins = Origins::default();
        let moved: Vec<(&Shape, f64)> = changes.iter().filter(|c| c.0 == b).map(|c| (&c.2, c.3)).collect();
        for (i, face) in old.faces.iter().enumerate() {
            let grown_face = grown.iter().find(|g| g.0 == i).map(|g| &g.1);
            for image in shell.sub_shapes(ShapeKind::Face) {
                match grown_face {
                    // Changed: a new transient id, tracked back to the old face (std sees the change that way).
                    Some(g) if lies_on(&samples(g), &image) && lies_on(&samples(&image), g) => {
                        origins.inherit(&image, face)
                    }
                    None if lies_on(&samples(&face.shape), &image) && lies_on(&samples(&image), &face.shape) => {
                        origins.same(&image, face)
                    }
                    _ => {}
                }
            }
        }
        let new_edges = shell.sub_shapes(ShapeKind::Edge);
        for edge in &old.edges {
            let shift = moved.iter().find(|(e, _)| e.is_same(&edge.shape));
            for image in &new_edges {
                let matches = match shift {
                    // Moved: parallel to where it was, as far off as it moved.
                    Some((_, distance)) => match (segment(&edge.shape), segment(image)) {
                        (Some((a, b2)), Some((c, d))) => {
                            let dir = normalize(sub(b2, a));
                            let along = normalize(sub(d, c));
                            matches!((dir, along), (Some(x), Some(y)) if dot(x, y).abs() > 1.0 - 1e-9)
                                && (norm(sub(ea_foot(a, dir.unwrap(), c), c)) - distance.abs()).abs() < 1e-7
                                && grown.iter().any(|g| lies_on(&samples(image), &g.1))
                        }
                        _ => false,
                    },
                    // Unmoved, or run on along its line: they overlap.
                    None => match (segment(&edge.shape), segment(image)) {
                        (Some((a, b2)), Some((c, d))) => {
                            let on_line = |p: Vec3| {
                                let dir = normalize(sub(b2, a)).unwrap_or(Vec3::new(1.0, 0.0, 0.0));
                                norm(sub(ea_foot(a, dir, p), p)) < 1e-7
                            };
                            on_line(c) && on_line(d) && {
                                let (m1, m2) = (scale(add(a, b2), 0.5), scale(add(c, d), 0.5));
                                distance_to_segment(m1, c, d) < 1e-7 || distance_to_segment(m2, a, b2) < 1e-7
                            }
                        }
                        _ => lies_on(&samples(&edge.shape), image) && lies_on(&samples(image), &edge.shape),
                    },
                };
                // Moved or run on: changed; otherwise the same edge.
                let unchanged =
                    shift.is_none() && lies_on(&samples(&edge.shape), image) && lies_on(&samples(image), &edge.shape);
                if matches && unchanged {
                    origins.same(image, edge);
                } else if matches {
                    origins.inherit(image, edge);
                }
            }
        }
        // Vertices: where they were, or the nearest new corner of a grown face.
        let new_vertices = shell.sub_shapes(ShapeKind::Vertex);
        let mut claimed = vec![false; new_vertices.len()];
        for vertex in &old.vertices {
            let Ok(p) = vertex.shape.vertex_point() else { continue };
            let exact = new_vertices.iter().position(|v| v.vertex_point().is_ok_and(|q| norm(sub(p, q)) < 1e-9));
            let reach = moved.iter().map(|m| m.1.abs()).fold(0.0, f64::max) * 4.0 + 1e-9;
            let found = exact.or_else(|| {
                new_vertices
                    .iter()
                    .enumerate()
                    .filter(|(j, v)| {
                        !claimed[*j]
                            && v.vertex_point().is_ok_and(|q| norm(sub(p, q)) < reach)
                            && !old.vertices.iter().any(|o| {
                                o.shape
                                    .vertex_point()
                                    .is_ok_and(|r| v.vertex_point().is_ok_and(|q| norm(sub(r, q)) < 1e-9))
                            })
                    })
                    .min_by(|(_, x), (_, y)| {
                        let d = |v: &Shape| v.vertex_point().map(|q| norm(sub(p, q))).unwrap_or(f64::MAX);
                        d(x).total_cmp(&d(y))
                    })
                    .map(|(j, _)| j)
            });
            if let Some(j) = found {
                claimed[j] = true;
                if exact == Some(j) {
                    origins.same(&new_vertices[j], vertex);
                } else {
                    origins.inherit(&new_vertices[j], vertex);
                }
            }
        }
        replacements.push((b, shell, origins));
    }
    let info = Info::new(id);
    let mut added = Vec::new();
    for (b, shell, origins) in replacements {
        added.push((model.bodies[b].clone(), shell, origins));
    }
    model.remove_bodies(&bodies);
    for (old, shell, origins) in added {
        model.add_body(NewBody::replacing(&old, shell), &origins, &info, &|_| None);
    }
    Ok(Value::UNDEFINED)
}
