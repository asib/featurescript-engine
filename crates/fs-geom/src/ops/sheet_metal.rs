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
    /// The two walls, the directions from the edge into each, and their normals there.
    walls: (Shape, Shape),
    into: (Vec3, Vec3),
    normals: (Vec3, Vec3),
    /// Its length flat: (inner radius + K-factor × thickness) × the angle it turns through.
    flat_length: f64,
    /// Laid flat (`sheetMetalUnfold`).
    unfolded: bool,
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

fn bend_of(
    def: &Body,
    edge_index: usize,
    radius: f64,
    front: f64,
    back: f64,
    k_factor: f64,
    unfolded: bool,
) -> R<Bend> {
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
    let n2 = normal_at(f2, m)?;
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
    let angle = std::f64::consts::PI - alpha;
    let flat_length = (radii.0 + k_factor * thickness) * angle;
    Ok(Bend {
        edge,
        sheet_radius,
        axis_origin,
        axis_direction: along,
        at: m,
        setback,
        radii,
        toward,
        walls: (f1.clone(), f2.clone()),
        into: (d1, d2),
        normals: (n1, n2),
        flat_length,
        unfolded,
    })
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
                // Round a cylindrical face of the definition (a rolled wall or rounded bend): the same axis.
                for (i, wall) in def.faces.iter().enumerate() {
                    let Ok(w) = wall.shape.surface_definition() else { continue };
                    if w.kind != 1 {
                        continue;
                    }
                    let parallel = norm(cross(s.frame.normal, w.frame.normal)) < 1e-9;
                    let offset = sub(s.frame.origin, w.frame.origin);
                    let apart = norm(sub(offset, scale(w.frame.normal, dot(offset, w.frame.normal))));
                    if parallel && apart < 1e-6 {
                        return def_ref(EntityType::Face, i);
                    }
                }
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
        // A vertex: the definition vertex nearest it, within reach of the material and bends around that vertex
        // (a corner or rip is picked by a vertex of the part).
        EntityType::Vertex => {
            let p = shape.vertex_point().ok()?;
            let around = bends.iter().map(|b| b.setback + b.radii.1).fold(0.0, f64::max);
            let limit = reach + around + 1e-4;
            def.vertices
                .iter()
                .enumerate()
                .filter_map(|(i, v)| Some((norm(sub(v.shape.vertex_point().ok()?, p)), i)))
                .filter(|(d, _)| *d <= limit)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .and_then(|(_, i)| def_ref(EntityType::Vertex, i))
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
        if !matches!(
            face.shape.surface_type().map_err(occt_error)?,
            occt::SurfaceType::Plane | occt::SurfaceType::Cylinder
        ) {
            return Err(error("@updateSheetMetalGeometry: walls other than planes and cylinders aren't supported yet"));
        }
    }
    let model_k_factor = attribute_value(&model_attribute, "k-factor").and_then(Value::as_number).unwrap_or(0.45);
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
                let k_factor = match attribute_value(joint, "k-factor").and_then(Value::as_number) {
                    Some(k) => k,
                    None => model_k_factor,
                };
                let unfolded = joint.get("unfolded").and_then(Value::as_bool) == Some(true);
                bends.push(bend_of(&def, i, radius, front, back, k_factor, unfolded)?);
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
    for vertex in &def.vertices {
        let style = sm_attribute(&vertex.attributes, "CORNER")
            .and_then(|c| attribute_value(c, "cornerStyle"))
            .and_then(Value::as_str);
        if let Some(style) = style.filter(|s| *s != "SIMPLE") {
            return Err(error(format!("@updateSheetMetalGeometry: {style} corner reliefs aren't supported yet")));
        }
    }
    let clearance = match attribute_value(&model_attribute, "minimalClearance") {
        Some(v) => values::length(v, "minimal clearance")?,
        None => 2e-5,
    };
    let solid = build_part(&def, &bends, &rips, front, back, clearance)?;

    // Replace the model's part, keeping its identity, or make one.
    // Replace the model's parts (one for each piece a rip cuts off), keeping their identities, or make them.
    let def_association = association(&def.attributes).cloned();
    let old: Vec<usize> =
        (0..model.bodies.len()).filter(|&b| is_part_of(&model.bodies[b], def_association.as_ref())).collect();
    let old_bodies: Vec<Rc<Body>> = old.iter().map(|&b| model.bodies[b].clone()).collect();
    model.remove_bodies(&old);
    let mut attributes = Attributes::default();
    if let Some(a) = &def_association {
        attributes.unnamed.push(a.clone());
    }
    let info = Info::new(id.clone());
    let pieces = solid.sub_shapes(ShapeKind::Solid);
    let pieces = if pieces.is_empty() { vec![solid] } else { pieces };
    let mut indices = Vec::new();
    for (k, piece) in pieces.into_iter().enumerate() {
        let new = match old_bodies.get(k) {
            Some(old_body) => {
                NewBody { attributes: Rc::new(attributes.clone()), ..NewBody::replacing(old_body, piece) }
            }
            None => NewBody {
                attributes: Rc::new(attributes.clone()),
                ..NewBody::new(piece, BodyType::Solid, info.clone())
            },
        };
        indices.push(model.add_body(new, &Origins::default(), &info, &|_| None));
    }
    // The definition may have moved in the list.
    let def = model
        .bodies
        .iter()
        .find(|b| b.tid == def.tid)
        .cloned()
        .ok_or_else(|| regen_error("SHEET_METAL_REBUILD_ERROR"))?;
    for index in indices {
        for kind in [EntityType::Face, EntityType::Edge, EntityType::Vertex] {
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
    }
    Ok(())
}

/// Whether a body is one of the parts of the model with this association attribute.
fn is_part_of(body: &Body, def_association: Option<&Value>) -> bool {
    body.body_type == BodyType::Solid && def_association.is_some() && association(&body.attributes) == def_association
}

/// Whether a model is active: sheet metal features work on it and keep its parts up to date (`sheetMetalEnd`
/// makes it inactive).
fn is_active(def: &Body) -> bool {
    sm_attribute(&def.attributes, "MODEL").and_then(|m| m.get("active")).and_then(Value::as_bool) != Some(false)
}

/// Whether an entity belongs to an active sheet metal model: its definition sheet, or one of its parts.
pub fn in_active_model(model: &Model, body: usize) -> bool {
    let b = &model.bodies[body];
    if b.is_sheet_metal_definition() {
        return is_active(b);
    }
    let Some(a) = association(&b.attributes) else { return false };
    model.bodies.iter().any(|d| d.is_sheet_metal_definition() && association(&d.attributes) == Some(a) && is_active(d))
}

/// A model made inactive leaves its parts as ordinary parts, without association attributes.
fn release_parts(model: &mut Model, def_index: usize) {
    let def_association = association(&model.bodies[def_index].attributes).cloned();
    for b in 0..model.bodies.len() {
        if !is_part_of(&model.bodies[b], def_association.as_ref()) {
            continue;
        }
        let strip = |attributes: &mut Attributes| attributes.unnamed.retain(|v| !is_type(v, "SMAssociationAttribute"));
        model.update_body(b, |body| {
            strip(Rc::make_mut(&mut body.attributes));
            for entity in body.faces.iter_mut().chain(body.edges.iter_mut()).chain(body.vertices.iter_mut()) {
                strip(Rc::make_mut(&mut entity.attributes));
            }
        });
    }
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

/// A planar wall cut back from some of its straight edges, each by its own distance: a strip that deep is taken
/// off along each edge, running on past corners but not into a straight continuation of the edge (a bend covering
/// part of an edge leaves the rest, such as the stubs a converted part's rips leave at its ends, batch 014).
fn cut_back(wall: &Shape, edges: &[Shape], distances: &[f64]) -> R<Shape> {
    let plane = wall.face_plane().map_err(occt_error)?;
    let n = plane.normal;
    let bounds = wall.bounding_box(false).map_err(occt_error)?;
    let far = norm(sub(bounds.max, bounds.min)) * 2.0 + 1e-3;
    let all_edges = wall.sub_shapes(ShapeKind::Edge);
    let mut strips = Vec::new();
    for (edge, &distance) in edges.iter().zip(distances) {
        let depth = -distance;
        let (Some((a, b)), Ok(tangent)) = (segment(edge), edge.edge_tangent(0.5, true)) else { continue };
        let Some(e) = normalize(sub(b, a)) else { continue };
        let into = into_face(wall, tangent.origin, e)?;
        // Run on past an end unless another edge carries straight on from it.
        let carries_on = |end: Vec3| {
            all_edges.iter().any(|other| {
                !other.is_same(edge)
                    && segment(other).is_some_and(|(c, d)| {
                        let touches = norm(sub(c, end)) < 1e-9 || norm(sub(d, end)) < 1e-9;
                        let off = |q: Vec3| norm(sub(sub(q, a), scale(e, dot(sub(q, a), e))));
                        touches && off(c) < 1e-9 && off(d) < 1e-9
                    })
            })
        };
        let length = norm(sub(b, a));
        let (u0, u1) = (if carries_on(a) { 0.0 } else { -far }, if carries_on(b) { length } else { length + far });
        let v_axis = cross(n, e);
        let (v0, v1) = if dot(v_axis, into) > 0.0 { (-far, depth) } else { (-depth, far) };
        let frame = occt::Frame { origin: a, normal: n, x: e };
        strips.push(occt::plane_face(frame, u0, u1, v0, v1).map_err(occt_error)?);
    }
    if strips.is_empty() {
        return Ok(wall.clone());
    }
    let op = occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(wall), &strips, true)
        .map_err(|_| regen_error("SHEET_METAL_REBUILD_ERROR"))?;
    Ok(op.shape())
}

/// A rigid motion: rotation rows and a translation.
#[derive(Clone, Copy)]
struct Rigid {
    r: [[f64; 3]; 3],
    t: Vec3,
}

impl Rigid {
    const IDENTITY: Rigid =
        Rigid { r: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], t: Vec3 { x: 0.0, y: 0.0, z: 0.0 } };

    fn vec(&self, v: Vec3) -> Vec3 {
        let r = &self.r;
        Vec3::new(
            r[0][0] * v.x + r[0][1] * v.y + r[0][2] * v.z,
            r[1][0] * v.x + r[1][1] * v.y + r[1][2] * v.z,
            r[2][0] * v.x + r[2][1] * v.y + r[2][2] * v.z,
        )
    }

    fn point(&self, p: Vec3) -> Vec3 {
        add(self.vec(p), self.t)
    }

    /// `self` after `other`.
    fn then(&self, other: &Rigid) -> Rigid {
        let mut r = [[0.0; 3]; 3];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, x) in row.iter_mut().enumerate() {
                *x = (0..3).map(|k| self.r[i][k] * other.r[k][j]).sum();
            }
        }
        Rigid { r, t: self.point(other.t) }
    }

    /// The motion taking the frame (origin, x, z) to another.
    fn between(from: (Vec3, Vec3, Vec3), to: (Vec3, Vec3, Vec3)) -> Rigid {
        let axes = |x: Vec3, z: Vec3| [x, cross(z, x), z];
        let (a, b) = (axes(from.1, from.2), axes(to.1, to.2));
        let mut r = [[0.0; 3]; 3];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, x) in row.iter_mut().enumerate() {
                let get = |v: Vec3, k: usize| [v.x, v.y, v.z][k];
                *x = (0..3).map(|k| get(b[k], i) * get(a[k], j)).sum();
            }
        }
        let m = Rigid { r, t: Vec3::new(0.0, 0.0, 0.0) };
        Rigid { r, t: sub(to.0, m.vec(from.0)) }
    }

    fn transform(&self) -> occt::Transform {
        let r = &self.r;
        occt::Transform {
            m: [
                r[0][0], r[0][1], r[0][2], self.t.x, r[1][0], r[1][1], r[1][2], self.t.y, r[2][0], r[2][1], r[2][2],
                self.t.z,
            ],
        }
    }
}

/// Where each wall goes when bends are unfolded: none for walls that stay put. Walls are reached from the held
/// wall (`holdFaceForUnfold`, else the first) across bends; across an unfolded bend, the far wall turns into the
/// near one's plane, a flat bend's length further on.
struct Placement {
    walls: Vec<Option<occt::Transform>>,
    /// For each unfolded bend's edge, the wall it unfolds from.
    from: Vec<(Shape, usize)>,
}

impl Placement {
    fn unfolded_from(&self, bend: &Bend, wall_index: &dyn Fn(&Shape) -> usize) -> (usize, usize) {
        let from = self
            .from
            .iter()
            .find(|(e, _)| e.is_same(&bend.edge))
            .map(|f| f.1)
            .unwrap_or_else(|| wall_index(&bend.walls.0));
        let other =
            if from == wall_index(&bend.walls.0) { wall_index(&bend.walls.1) } else { wall_index(&bend.walls.0) };
        (from, other)
    }
}

fn placements(def: &Body, bends: &[Bend], wall_index: &dyn Fn(&Shape) -> usize) -> Placement {
    let n = def.faces.len();
    if !bends.iter().any(|b| b.unfolded) {
        return Placement { walls: vec![None; n], from: Vec::new() };
    }
    let held = def
        .faces
        .iter()
        .position(|f| {
            sm_attribute(&f.attributes, "WALL").and_then(|w| w.get("holdFaceForUnfold")).and_then(Value::as_bool)
                == Some(true)
        })
        .unwrap_or(0);
    let mut motion: Vec<Option<Rigid>> = vec![None; n];
    motion[held] = Some(Rigid::IDENTITY);
    let mut from = Vec::new();
    let mut queue = vec![held];
    while let Some(w) = queue.pop() {
        let here = motion[w].expect("placed");
        for bend in bends {
            let (a, b) = (wall_index(&bend.walls.0), wall_index(&bend.walls.1));
            let (near, far, first) = if a == w {
                (a, b, true)
            } else if b == w {
                (b, a, false)
            } else {
                continue;
            };
            if motion[far].is_some() {
                continue;
            }
            let there = if bend.unfolded {
                from.push((bend.edge.clone(), near));
                let leaves = |toward: Vec3| add(bend.axis_origin, scale(toward, bend.sheet_radius));
                let (t_near, d_near, n_near, t_far, d_far, n_far) = if first {
                    (
                        leaves(bend.toward.0),
                        bend.into.0,
                        bend.normals.0,
                        leaves(bend.toward.1),
                        bend.into.1,
                        bend.normals.1,
                    )
                } else {
                    (
                        leaves(bend.toward.1),
                        bend.into.1,
                        bend.normals.1,
                        leaves(bend.toward.0),
                        bend.into.0,
                        bend.normals.0,
                    )
                };
                let target = add(t_near, scale(d_near, -bend.flat_length));
                here.then(&Rigid::between((t_far, d_far, n_far), (target, scale(d_near, -1.0), n_near)))
            } else {
                here
            };
            motion[far] = Some(there);
            queue.push(far);
        }
    }
    let is_identity = |m: &Rigid| {
        (0..3).all(|i| (0..3).all(|j| (m.r[i][j] - if i == j { 1.0 } else { 0.0 }).abs() < 1e-12)) && norm(m.t) < 1e-12
    };
    Placement { walls: motion.iter().map(|m| m.filter(|m| !is_identity(m)).map(|m| m.transform())).collect(), from }
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
    let wall_index =
        |shape: &Shape| def.faces.iter().position(|f| f.shape.is_same(shape)).expect("a wall of the model");
    let placement = placements(def, bends, &wall_index);
    let place = |piece: Shape, wall: usize| -> R<Shape> {
        match &placement.walls[wall] {
            Some(t) => occt::transform(&piece, t).map(|op| op.shape()).map_err(occt_error),
            None => Ok(piece),
        }
    };
    let mut pieces: Vec<Shape> = Vec::new();
    let mut cut_walls: Vec<Shape> = Vec::new();
    for (w, wall) in def.faces.iter().enumerate() {
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
        // A rolled wall, or a bend the model already rounds (std's bend wraps one in), is thickened as it is.
        let cut = if edges.is_empty() || wall.shape.surface_type().ok() != Some(occt::SurfaceType::Plane) {
            wall.shape.clone()
        } else {
            cut_back(&wall.shape, &edges, &distances)?
        };
        for face in cut.sub_shapes(ShapeKind::Face) {
            let solid =
                occt::thicken(&face, front, back).map_err(|_| regen_error("SHEET_METAL_CANNOT_THICKEN"))?.shape();
            pieces.push(place(solid, w)?);
        }
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
        if bend.unfolded {
            // Laid flat: a strip as long as the bend is flat, in the plane of the wall it's unfolded from.
            let (from, _) = placement.unfolded_from(bend, &wall_index);
            let (toward, into, normal) = if from == wall_index(&bend.walls.0) {
                (bend.toward.0, bend.into.0, bend.normals.0)
            } else {
                (bend.toward.1, bend.into.1, bend.normals.1)
            };
            let start =
                add(add(bend.axis_origin, scale(toward, bend.sheet_radius)), scale(bend.axis_direction, span.0));
            let away = scale(into, -1.0);
            let v_axis = cross(normal, bend.axis_direction);
            let (v0, v1) = if dot(v_axis, away) > 0.0 { (0.0, bend.flat_length) } else { (-bend.flat_length, 0.0) };
            let frame = occt::Frame { origin: start, normal, x: bend.axis_direction };
            let strip = occt::plane_face(frame, 0.0, span.1 - span.0, v0, v1).map_err(occt_error)?;
            let solid =
                occt::thicken(&strip, front, back).map_err(|_| regen_error("SHEET_METAL_CANNOT_THICKEN"))?.shape();
            pieces.push(place(solid, from)?);
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
        pieces.push(place(piece, wall_index(&bend.walls.0))?);
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
        if is_active(&model.bodies[index]) {
            regenerate(model, &id, index)?;
        } else {
            release_parts(model, index);
        }
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

/// `@evCornerType`: what kind of corner a definition vertex is. One bend ending there is a bend end; two or more
/// bends meeting there with a rip between their walls make a closed corner, without one an open corner.
pub fn corner_type(tags: &crate::values::Tags, model: &Model, definition: &Value) -> R<Value> {
    let found = query::evaluate(model, field(definition, "vertex"))?;
    let [r] = found[..] else { return Err(regen_error("BAD_GEOMETRY")) };
    if r.kind != EntityType::Vertex {
        return Err(regen_error("BAD_GEOMETRY"));
    }
    let body = &model.bodies[r.body];
    let vertex = &body.vertices[r.index].shape;
    let joint_at = |kind: &str| {
        body.edges
            .iter()
            .filter(|e| e.shape.sub_shapes(ShapeKind::Vertex).iter().any(|v| v.is_same(vertex)))
            .filter(|e| {
                sm_attribute(&e.attributes, "JOINT")
                    .and_then(|j| attribute_value(j, "jointType"))
                    .and_then(Value::as_str)
                    == Some(kind)
            })
            .count()
    };
    let (bends, rips) = (joint_at("BEND"), joint_at("RIP"));
    let corner = match (bends, rips) {
        (0, _) => "NOT_A_CORNER",
        (1, 0) => "BEND_END",
        (_, 0) => "OPEN_CORNER",
        _ => "CLOSED_CORNER",
    };
    let tag = fs_interp::types::find("SMCornerType").ok_or_else(|| error("std type SMCornerType isn't loaded"))?;
    // Transient ids, which std's evCornerType turns into queries.
    let transient = Value::string(query::tid_string(body.vertices[r.index].tid));
    Ok(crate::values::map(vec![
        ("cornerType", tags.enum_value(tag, corner)),
        ("primaryVertex", transient.clone()),
        ("allVertices", Value::array(vec![transient])),
    ]))
}

/// `@sheetMetalApplyInFlat`: a corner break (std's fillet or chamfer of sheet metal corners) applied to the model
/// itself: each wall's corner at the definition vertices the picks are associated with is rounded or chamfered in
/// the wall's plane. Faces, edges and vertices the change leaves alone keep their identities; the corner's walls and
/// the edges it shortens are changed. Only circular fillets and equal-offset chamfers for now.
pub fn apply_in_flat(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let chamfer = field(definition, "crossSection").as_str() == Some("CHAMFER");
    let size = if chamfer {
        if field(definition, "chamferType").as_str().is_some_and(|t| t != "EQUAL_OFFSETS") {
            return Err(error("@sheetMetalApplyInFlat: only equal-offset chamfers are supported yet"));
        }
        values::length(field(definition, "width"), "corner break width")?
    } else {
        if field(definition, "crossSection").as_str().is_some_and(|c| c != "CIRCULAR") {
            return Err(error("@sheetMetalApplyInFlat: only circular fillets are supported yet"));
        }
        values::length(field(definition, "radius"), "corner break radius")?
    };
    // The definition vertices the picks are associated with, by body.
    let mut corners: Vec<(usize, Shape)> = Vec::new();
    for r in query::evaluate(model, field(definition, "entities"))? {
        let Some(a) = association(model.attributes(r)).cloned() else { continue };
        for (b, body) in model.bodies.iter().enumerate() {
            if !body.is_sheet_metal_definition() {
                continue;
            }
            for v in &body.vertices {
                if association(&v.attributes) == Some(&a) && !corners.iter().any(|(_, s)| s.is_same(&v.shape)) {
                    corners.push((b, v.shape.clone()));
                }
            }
        }
    }
    if corners.is_empty() {
        return Err(regen_error("SHEET_METAL_CORNER_BREAK_SELECT_ENTITIES"));
    }
    let mut bodies: Vec<usize> = corners.iter().map(|c| c.0).collect();
    bodies.sort_unstable();
    bodies.dedup();
    let mut replacements = Vec::new();
    for &b in &bodies {
        let old = model.bodies[b].clone();
        let mut faces: Vec<Shape> = old.faces.iter().map(|f| f.shape.clone()).collect();
        let mut changed: Vec<usize> = Vec::new();
        for (i, face) in old.faces.iter().enumerate() {
            let at: Vec<Shape> = corners
                .iter()
                .filter(|(cb, v)| *cb == b && face.shape.sub_shapes(ShapeKind::Vertex).iter().any(|x| x.is_same(v)))
                .map(|(_, v)| v.clone())
                .collect();
            if at.is_empty() {
                continue;
            }
            faces[i] = occt::face_corners_2d(&face.shape, &at, size, chamfer)
                .map_err(|_| regen_error("SHEET_METAL_CORNER_BREAK_FAILED"))?;
            changed.push(i);
        }
        let shell = occt::sew_faces(&faces, 1e-7).map_err(occt_error)?;
        let mut origins = Origins::default();
        for (i, face) in old.faces.iter().enumerate() {
            let image = shell.sub_shapes(ShapeKind::Face).into_iter().find(|f| {
                let target = if changed.contains(&i) { &faces[i] } else { &face.shape };
                lies_on(&samples(target), f) && lies_on(&samples(f), target)
            });
            if let Some(image) = image {
                if changed.contains(&i) { origins.inherit(&image, face) } else { origins.same(&image, face) }
            }
        }
        for edge in &old.edges {
            for image in shell.sub_shapes(ShapeKind::Edge) {
                let same = lies_on(&samples(&edge.shape), &image) && lies_on(&samples(&image), &edge.shape);
                // Shortened by the corner: along the old edge, and not all of it.
                let shortened = !same && lies_on(&samples(&image), &edge.shape);
                if same {
                    origins.same(&image, edge);
                } else if shortened {
                    origins.inherit(&image, edge);
                }
            }
        }
        for vertex in &old.vertices {
            let Ok(p) = vertex.shape.vertex_point() else { continue };
            if let Some(image) = shell
                .sub_shapes(ShapeKind::Vertex)
                .into_iter()
                .find(|v| v.vertex_point().is_ok_and(|q| norm(sub(p, q)) < 1e-9))
            {
                origins.same(&image, vertex);
            }
        }
        replacements.push((old, shell, origins));
    }
    model.remove_bodies(&bodies);
    let info = Info::new(id);
    // The changed model is a new body, tracked back to the old: std finds it by tracking the old one.
    for (old, shell, origins) in replacements {
        model.add_body(NewBody { tid: None, ..NewBody::replacing(&old, shell) }, &origins, &info, &|_| None);
    }
    Ok(Value::UNDEFINED)
}
