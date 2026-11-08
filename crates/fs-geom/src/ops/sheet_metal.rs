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

/// The transient ids of the bodies registered to cut a wall (`cuttingToolBodyIds` of its wall attribute).
fn wall_tool_tids(attributes: &Attributes) -> Vec<crate::model::Tid> {
    sm_attribute(attributes, "WALL")
        .and_then(|a| a.get("cuttingToolBodyIds"))
        .and_then(Value::as_array)
        .map(|ids| ids.iter().filter_map(|v| v.as_str().and_then(crate::query::parse_tid)).collect())
        .unwrap_or_default()
}

/// The forms registered on a wall, as the transient ids of their parts to add and to remove.
pub(crate) fn wall_form_parts(attributes: &Attributes) -> Vec<(Option<crate::model::Tid>, Option<crate::model::Tid>)> {
    wall_forms(attributes).into_iter().map(|f| (f.positive, f.negative)).collect()
}

/// `wall_tool_tids` for other modules.
pub(crate) fn wall_tool_tids_of(attributes: &Attributes) -> Vec<crate::model::Tid> {
    wall_tool_tids(attributes)
}

/// A form registered on a wall (`formedToolBodyIds` of its wall attribute): the transient ids of its part to add, its
/// part to remove and its placement mate connector.
struct Form {
    positive: Option<crate::model::Tid>,
    negative: Option<crate::model::Tid>,
    placement: Option<crate::model::Tid>,
}

fn wall_forms(attributes: &Attributes) -> Vec<Form> {
    let id = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).and_then(crate::query::parse_tid);
    sm_attribute(attributes, "WALL")
        .and_then(|a| a.get("formedToolBodyIds"))
        .and_then(Value::as_array)
        .map(|forms| {
            forms
                .iter()
                .map(|f| Form {
                    positive: id(f, "positiveBodyId"),
                    negative: id(f, "negativeBodyId"),
                    placement: id(f, "placementBodyId"),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The bodies registered to cut or form sheet metal walls. Like the definition sheets, they belong to the model and
/// are hidden outside sheet metal features (Onshape doesn't count them as parts, batches 017 and 022). A form's
/// placement mate connector stays.
pub fn cutting_tools(model: &Model) -> Vec<crate::model::Tid> {
    model
        .bodies
        .iter()
        .filter(|b| b.is_sheet_metal_definition())
        .flat_map(|b| {
            b.faces.iter().flat_map(|f| {
                let forms =
                    wall_forms(&f.attributes).into_iter().flat_map(|form| [form.positive, form.negative]).flatten();
                wall_tool_tids(&f.attributes).into_iter().chain(forms)
            })
        })
        .collect()
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
fn associate(def: &Body, bends: &[Bend], front: f64, back: f64, kind: EntityType, shape: &Shape) -> Option<EntityRef> {
    let thickness = front + back;
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
            // Across the thickness at a curved boundary edge (the wall of a hole, batch 017): the face holds the
            // edge carried into the material.
            let mid = if (front - back).abs() > 1e-9 { (front - back) / 2.0 } else { thickness / 4.0 };
            for &i in &boundary {
                let e = &def.edges[i].shape;
                if segment(e).is_some() {
                    continue;
                }
                let Some(wall) =
                    def.faces.iter().find(|f| f.shape.sub_shapes(ShapeKind::Edge).iter().any(|x| x.is_same(e)))
                else {
                    continue;
                };
                let Ok(plane) = wall.shape.face_plane() else { continue };
                let points = samples(e);
                let on = |q: Vec3| occt::vertex(q).and_then(|v| v.distance(shape)).is_ok_and(|d| d.distance < 1e-6);
                for side in [mid, -mid] {
                    if !points.is_empty() && points.iter().all(|&q| on(add(q, scale(plane.normal, side)))) {
                        return def_ref(EntityType::Edge, i);
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
    // The parts are tied to their definition sheet by its association attribute. std gives the sheets it makes
    // one; a sheet patterned as a part (batch 017) gets one here, as std would name it.
    if association(&model.bodies[def_index].attributes).is_none()
        && let Some(tag) = fs_interp::types::find("SMAssociationAttribute")
    {
        let name = format!(
            "{{ queryType : TRANSIENT , transientId : {} }}",
            crate::query::tid_string(model.bodies[def_index].tid)
        );
        let mut attribute = values::map(vec![("attributeId", Value::string(name))]);
        attribute.tag = Some(tag);
        model.update_body(def_index, |body| Rc::make_mut(&mut body.attributes).unnamed.push(attribute));
    }
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
                if !matches!(style, "EDGE" | "BUTT" | "BUTT2") {
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
    let scale =
        |key: &str, default: f64| attribute_value(&model_attribute, key).and_then(Value::as_number).unwrap_or(default);
    let relief = BendRelief {
        style: model_attribute.get("defaultBendReliefStyle").and_then(Value::as_str).unwrap_or("OBROUND").to_string(),
        depth_scale: scale("defaultBendReliefDepthScale", 1.5),
        width_scale: scale("defaultBendReliefScale", 1.0625),
        thickness,
        depth: 0.0,
    };
    let corner_defaults = CornerDefaults {
        two: model_attribute.get("defaultTwoCornerStyle").and_then(Value::as_str).unwrap_or("SIMPLE").to_string(),
        scale: scale("defaultCornerReliefScale", 1.5),
        diameter: attribute_value(&model_attribute, "defaultRoundReliefDiameter")
            .map(|v| values::length(v, "relief diameter"))
            .transpose()?
            .unwrap_or(0.0),
        width: attribute_value(&model_attribute, "defaultSquareReliefWidth")
            .map(|v| values::length(v, "relief width"))
            .transpose()?
            .unwrap_or(0.0),
    };
    let corners = corner_reliefs(&def, &bends, &corner_defaults, front, back, clearance)?;
    // The bodies registered to cut each wall: besides the outline they leave in the definition, they're cut from
    // the wall's piece of the part, which takes what doesn't go through it (a counterbore, batch 018).
    let tools: Vec<Vec<Shape>> = def
        .faces
        .iter()
        .map(|f| {
            wall_tool_tids(&f.attributes)
                .iter()
                .filter_map(|t| model.bodies.iter().find(|b| b.tid == *t).map(|b| b.shape.clone()))
                .collect()
        })
        .collect();
    // The forms on each wall: what each removes, and what it adds (batch 022).
    let body_of = |t: Option<crate::model::Tid>| {
        t.and_then(|t| model.bodies.iter().find(|b| b.tid == t)).map(|b| b.shape.clone())
    };
    let forms: Vec<(Vec<Shape>, Vec<Shape>)> = def
        .faces
        .iter()
        .map(|f| {
            let forms = wall_forms(&f.attributes);
            (
                forms.iter().filter_map(|form| body_of(form.negative)).collect(),
                forms.iter().filter_map(|form| body_of(form.positive)).collect(),
            )
        })
        .collect();
    let solid = build_part(&def, &bends, &rips, front, back, clearance, &relief, &corners, &tools, &forms)?;

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
    // The registered tools' faces, whose info the part's faces they cut take (a countersunk hole's cone and
    // cylinder, which std finds by the names `@opHole` gave them, batch 032).
    // Forms' parts too (batch 033): a form's faces are made by the form.
    let tool_faces: Vec<(Shape, Rc<Info>)> = def
        .faces
        .iter()
        .flat_map(|f| {
            let forms = wall_forms(&f.attributes).into_iter().flat_map(|form| [form.positive, form.negative]).flatten();
            wall_tool_tids(&f.attributes).into_iter().chain(forms).collect::<Vec<_>>()
        })
        .filter_map(|t| model.bodies.iter().find(|b| b.tid == t))
        .flat_map(|b| b.faces.iter().map(|f| (f.shape.clone(), f.info.clone())))
        .collect();
    for index in indices {
        for kind in [EntityType::Face, EntityType::Edge, EntityType::Vertex] {
            for i in 0..model.bodies[index].entities(kind).len() {
                let shape = model.bodies[index].entities(kind)[i].shape.clone();
                if kind == EntityType::Face
                    && let Some(info) = tool_face_info(&tool_faces, &shape)
                {
                    model.update_body(index, |body| body.faces[i].info = info);
                    continue;
                }
                let Some(from) = associate(&def, &bends, front, back, kind, &shape) else { continue };
                let source = &def.entities(from.kind)[from.index];
                // The part's entities are made by what made the definition's (a hole's wall by the cut that made
                // the hole's edge, batch 017).
                let made_by = source.info.clone();
                model.update_body(index, |body| {
                    let entities = match kind {
                        EntityType::Face => &mut body.faces,
                        EntityType::Edge => &mut body.edges,
                        _ => &mut body.vertices,
                    };
                    entities[i].info = made_by;
                });
                let Some(a) = association(&source.attributes).cloned() else { continue };
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

/// The definition sheet of a sheet metal part, if it is one.
pub fn definition_of_part(model: &Model, body: usize) -> Option<usize> {
    let b = &model.bodies[body];
    if b.is_sheet_metal_definition() {
        return None;
    }
    let a = association(&b.attributes)?;
    model.bodies.iter().position(|d| d.is_sheet_metal_definition() && association(&d.attributes) == Some(a))
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
    // Walls meeting tangentially (a flat wall running on into a rolled one, batch 043) are each cut back by half the
    // clearance.
    if c.abs() < 1e-6 {
        return Ok(clearance / 2.0);
    }
    let reach = (front / c).max(-back / c).max(0.0);
    Ok(reach + clearance)
}

/// How far a wall runs on past a butt joint's edge to cover the other wall's material (negative: a setback the
/// other way).
fn overlap_setback(wall: &Shape, other: &Shape, edge: &Shape, front: f64, back: f64) -> R<f64> {
    let tangent = edge.edge_tangent(0.5, true).map_err(occt_error)?;
    let (m, along) = (tangent.origin, tangent.direction);
    let into = into_face(wall, m, along)?;
    let n = normal_at(other, m)?;
    let c = dot(into, n);
    if c.abs() < 1e-12 {
        return Ok(0.0);
    }
    Ok((front / c).min(-back / c).min(0.0))
}

/// A planar wall cut back from some of its straight edges, each by its own distance: a strip that deep is taken
/// off along each edge, running on past corners but not into a straight continuation of the edge (a bend covering
/// part of an edge leaves the rest, such as the stubs a converted part's rips leave at its ends, batch 014).
fn cut_back(wall: &Shape, edges: &[Shape], distances: &[f64], extra: &[Shape]) -> R<Shape> {
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
    strips.extend(extra.iter().cloned());
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

    fn inverse(&self) -> Rigid {
        let mut r = [[0.0; 3]; 3];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, x) in row.iter_mut().enumerate() {
                *x = self.r[j][i];
            }
        }
        let back = Rigid { r, t: Vec3::new(0.0, 0.0, 0.0) };
        Rigid { r, t: scale(back.vec(self.t), -1.0) }
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

/// The motion laying the wall on the far side of a bend into the near wall's plane, a flat bend's length on.
fn unfolding(bend: &Bend, near_is_first: bool) -> Rigid {
    let leaves = |toward: Vec3| add(bend.axis_origin, scale(toward, bend.sheet_radius));
    let (t_near, d_near, n_near, t_far, d_far, n_far) = if near_is_first {
        (leaves(bend.toward.0), bend.into.0, bend.normals.0, leaves(bend.toward.1), bend.into.1, bend.normals.1)
    } else {
        (leaves(bend.toward.1), bend.into.1, bend.normals.1, leaves(bend.toward.0), bend.into.0, bend.normals.0)
    };
    let target = add(t_near, scale(d_near, -bend.flat_length));
    Rigid::between((t_far, d_far, n_far), (target, scale(d_near, -1.0), n_near))
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
                here.then(&unfolding(bend, first))
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
/// The model's default corner relief: its style where two bends meet, and sizes.
struct CornerDefaults {
    two: String,
    scale: f64,
    diameter: f64,
    width: f64,
}

/// A corner relief: where two bends meet on a base wall, a hole cut in the flat pattern (the base wall's plane)
/// and folded back through the base wall, the bends and the walls beyond them.
struct CornerRelief {
    /// The bends (indices into the model's bends) and the base wall they share.
    bends: (usize, usize),
    base: Shape,
    /// The hole, as a face in the base wall's plane.
    hole: Shape,
    /// A closed corner: what each bend loses in the flat (by bend index), the hole only taking the base wall's
    /// corner; the walls beyond keep their rip setbacks.
    closed: Vec<(usize, Shape)>,
}

/// The corner reliefs a model has: at each vertex where two bends meet, with the vertex's own corner style or the
/// model's. Centred where the bends' centre lines cross in the flat (each bend's tangent line on the base wall plus
/// half its flat length): round, a circle of radius scale × flat length / √2; rectangular, a square of side scale ×
/// flat length; sized, the given diameter or width (fitted to fixture batch 016). Closed corners aren't supported
/// yet.
fn corner_reliefs(
    def: &Body,
    bends: &[Bend],
    defaults: &CornerDefaults,
    front: f64,
    back: f64,
    clearance: f64,
) -> R<Vec<CornerRelief>> {
    let mut out = Vec::new();
    for vertex in &def.vertices {
        let at: Vec<usize> = (0..bends.len())
            .filter(|&i| bends[i].edge.sub_shapes(ShapeKind::Vertex).iter().any(|v| v.is_same(&vertex.shape)))
            .collect();
        if at.len() < 2 {
            continue;
        }
        let corner = sm_attribute(&vertex.attributes, "CORNER");
        let own = |key: &str| corner.and_then(|c| attribute_value(c, key));
        let style =
            own("cornerStyle").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| defaults.two.clone());
        if style == "SIMPLE" {
            continue;
        }
        if at.len() > 2 {
            return Err(error("@updateSheetMetalGeometry: reliefs where three bends meet aren't supported yet"));
        }
        let (i, j) = (at[0], at[1]);
        let (b1, b2) = (&bends[i], &bends[j]);
        // The wall both bends leave.
        let base = [&b1.walls.0, &b1.walls.1]
            .into_iter()
            .find(|w| w.is_same(&b2.walls.0) || w.is_same(&b2.walls.1))
            .ok_or_else(|| error("@updateSheetMetalGeometry: a corner relief needs the bends to share a wall"))?
            .clone();
        let leaves = |b: &Bend| -> (Vec3, Vec3, Vec3) {
            let (toward, into, normal) = if b.walls.0.is_same(&base) {
                (b.toward.0, b.into.0, b.normals.0)
            } else {
                (b.toward.1, b.into.1, b.normals.1)
            };
            (add(b.axis_origin, scale(toward, b.sheet_radius)), scale(into, -1.0), normal)
        };
        let (t1, away1, n) = leaves(b1);
        let (t2, away2, _) = leaves(b2);
        let c1 = add(t1, scale(away1, b1.flat_length / 2.0));
        let c2 = add(t2, scale(away2, b2.flat_length / 2.0));
        // Where c1 + s·axis1 meets c2 + u·axis2.
        let (d1, d2) = (b1.axis_direction, b2.axis_direction);
        let w = sub(c2, c1);
        let cr = cross(d1, d2);
        let denom = dot(cr, cr);
        if denom < 1e-18 {
            return Err(error("@updateSheetMetalGeometry: a corner relief between parallel bends"));
        }
        let centre = add(c1, scale(d1, dot(cross(w, d2), cr) / denom));
        let flat = b1.flat_length.max(b2.flat_length);
        let number = |key: &str, default: f64| own(key).and_then(Value::as_number).unwrap_or(default);
        let length = |key: &str, default: f64| -> R<f64> {
            match own(key) {
                Some(v) => values::length(v, "corner relief size"),
                None => Ok(default),
            }
        };
        if style == "CLOSED" {
            out.push(closed_corner(
                def,
                vertex,
                (i, j),
                base,
                (t1, away1),
                (t2, away2),
                n,
                bends,
                front,
                back,
                clearance,
            )?);
            continue;
        }
        let frame = occt::Frame { origin: centre, normal: n, x: d1 };
        let hole = match style.as_str() {
            "ROUND" | "SIZED_ROUND" => {
                let radius = if style == "ROUND" {
                    number("cornerReliefScale", defaults.scale) * flat / std::f64::consts::SQRT_2
                } else {
                    length("roundReliefDiameter", defaults.diameter)? / 2.0
                };
                let circle = occt::circle_edge(frame, radius).map_err(occt_error)?;
                occt::fill_edges(&[circle]).map_err(occt_error)?
            }
            "RECTANGLE" | "SIZED_RECTANGLE" => {
                let side = if style == "RECTANGLE" {
                    number("cornerReliefScale", defaults.scale) * flat
                } else {
                    length("squareReliefWidth", defaults.width)?
                };
                occt::plane_face(frame, -side / 2.0, side / 2.0, -side / 2.0, side / 2.0).map_err(occt_error)?
            }
            other => {
                return Err(error(format!("@updateSheetMetalGeometry: {other} corner reliefs aren't supported yet")));
            }
        };
        out.push(CornerRelief { bends: (i, j), base, hole, closed: Vec::new() });
    }
    Ok(out)
}

/// A closed corner (batch 016, c013): each bend runs on to the corner, its end where the plane through two points,
/// parallel to the base wall's normal, cuts the sheet: on the bend's tangent line on the base wall, just short of where
/// the two tangent lines meet, and where the wall beyond it stops (its rip setback). The part's end face runs across
/// the thickness from there (Onshape's B-spline face): in the flat, the cut is a sine curve. The base wall's corner is
/// cut square to the corner's diagonal so the two bends stay the minimal clearance apart there.
#[allow(clippy::too_many_arguments)]
fn closed_corner(
    def: &Body,
    vertex: &crate::model::Entity,
    (i, j): (usize, usize),
    base: Shape,
    (t1, away1): (Vec3, Vec3),
    (t2, away2): (Vec3, Vec3),
    normal: Vec3,
    bends: &[Bend],
    front: f64,
    back: f64,
    clearance: f64,
) -> R<CornerRelief> {
    let failed = || error("@updateSheetMetalGeometry: a closed corner whose bends don't meet");
    let (b1, b2) = (&bends[i], &bends[j]);
    let (d1, d2) = (b1.axis_direction, b2.axis_direction);
    // Where the two tangent lines meet.
    let w = sub(t2, t1);
    let cr = cross(d1, d2);
    let denom = dot(cr, cr);
    if denom < 1e-18 {
        return Err(failed());
    }
    let p = add(t1, scale(d1, dot(cross(w, d2), cr) / denom));
    let v = vertex.shape.vertex_point().map_err(occt_error)?;
    let g = normalize(sub(sub(v, scale(normal, dot(sub(v, p), normal))), p)).ok_or_else(failed)?;
    // Along each tangent line, away from the corner.
    let e1 = if dot(d1, g) < 0.0 { d1 } else { scale(d1, -1.0) };
    let e2 = if dot(d2, g) < 0.0 { d2 } else { scale(d2, -1.0) };
    let half = norm(add(e1, e2)) / 2.0;
    let delta = (clearance / 2.0) / half.max(1e-9);
    let (a1, a2) = (add(p, scale(e1, delta)), add(p, scale(e2, delta)));
    // Far enough to cover the corner, but not so far the regions' tiny edges get lost.
    let reach = (b1.flat_length + b2.flat_length) * 4.0 + 1e-3;
    let polygon = |points: &[Vec3]| -> R<Shape> {
        let edges = (0..points.len())
            .map(|k| occt::line_edge(points[k], points[(k + 1) % points.len()]).map_err(occt_error))
            .collect::<R<Vec<_>>>()?;
        let x = normalize(sub(points[1], points[0])).ok_or_else(failed)?;
        occt::sketch_regions(&edges, occt::Frame { origin: points[0], normal, x })
            .map_err(occt_error)?
            .into_iter()
            .next()
            .ok_or_else(failed)
    };
    let hole = polygon(&[a1, add(a1, scale(g, reach)), add(a2, scale(g, reach)), a2])?;
    // Each bend: the wall beyond it stops its rip setback short of the corner.
    let wall_beyond = |b: &Bend| if b.walls.0.is_same(&base) { b.walls.1.clone() } else { b.walls.0.clone() };
    let (w1, w2) = (wall_beyond(b1), wall_beyond(b2));
    let rip = def.edges.iter().map(|e| &e.shape).find(|e| {
        let on = |f: &Shape| f.sub_shapes(ShapeKind::Edge).iter().any(|x| x.is_same(e));
        on(&w1) && on(&w2)
    });
    let setback = |wall: &Shape, other: &Shape| -> R<f64> {
        match rip {
            Some(edge) => rip_setback(wall, other, edge, front, back, clearance),
            None => Ok(clearance),
        }
    };
    let (s1, s2) = (setback(&w1, &w2)?, setback(&w2, &w1)?);
    let cut = |bend: &Bend, t: Vec3, away: Vec3, d: Vec3, e: Vec3, a: Vec3, s: f64| -> R<Shape> {
        let flat = bend.flat_length;
        let at_corner = add(t, scale(d, dot(sub(v, t), d)));
        let b = add(add(at_corner, scale(away, flat)), scale(e, s));
        // Along the bend, the cut moves by its share of the way from a to b as the sine of the angle turned.
        let turn = std::f64::consts::PI - dot(bend.into.0, bend.into.1).clamp(-1.0, 1.0).acos();
        let along = dot(sub(b, a), d) / turn.sin();
        let n = 24;
        let mut points = Vec::new();
        let mut params = Vec::new();
        for k in 0..=n {
            let f = k as f64 / n as f64;
            points.push(add(add(a, scale(away, flat * f)), scale(d, along * (turn * f).sin())));
            params.push(f);
        }
        let derivative = |f: f64| add(scale(away, flat), scale(d, along * turn * (turn * f).cos()));
        let curve = crate::splines::spline(
            &points,
            crate::splines::Ends::Clamped(derivative(0.0), derivative(1.0)),
            Some(params),
        )?;
        let (far_b, far_a) = (sub(b, scale(e, reach)), sub(a, scale(e, reach)));
        let edges = vec![
            curve,
            occt::line_edge(b, far_b).map_err(occt_error)?,
            occt::line_edge(far_b, far_a).map_err(occt_error)?,
            occt::line_edge(far_a, a).map_err(occt_error)?,
        ];
        let x = normalize(away).ok_or_else(failed)?;
        occt::sketch_regions(&edges, occt::Frame { origin: a, normal, x })
            .map_err(occt_error)?
            .into_iter()
            .next()
            .ok_or_else(failed)
    };
    let closed = vec![(i, cut(b1, t1, away1, d1, e1, a1, s1)?), (j, cut(b2, t2, away2, d2, e2, a2, s2)?)];
    Ok(CornerRelief { bends: (i, j), base, hole, closed })
}

/// How the ends of a bend shorter than its wall's edge are relieved (the model's defaults, or a bend end's own).
#[derive(Clone)]
struct BendRelief {
    style: String,
    depth_scale: f64,
    width_scale: f64,
    thickness: f64,
    /// A sized relief's depth, in from where the wall is cut back for the bend.
    depth: f64,
}

impl BendRelief {
    /// The relief at a bend end: the one `sheetMetalBendRelief` set on the definition vertex there, if any.
    fn at(&self, def: &Body, end: Vec3) -> R<BendRelief> {
        let corner = def.vertices.iter().find_map(|v| {
            let p = v.shape.vertex_point().ok()?;
            if norm(sub(p, end)) > 1e-7 {
                return None;
            }
            sm_attribute(&v.attributes, "CORNER").filter(|c| attribute_value(c, "cornerStyle").is_some())
        });
        let Some(corner) = corner else { return Ok(self.clone()) };
        let mut out = self.clone();
        if let Some(style) = attribute_value(corner, "cornerStyle").and_then(Value::as_str) {
            out.style = style.to_string();
        }
        if let Some(k) = attribute_value(corner, "bendReliefDepthScale").and_then(Value::as_number) {
            out.depth_scale = k;
        }
        if let Some(k) = attribute_value(corner, "bendReliefScale").and_then(Value::as_number) {
            out.width_scale = k;
        }
        if let Some(d) = attribute_value(corner, "bendReliefDepth") {
            out.depth = values::length(d, "bend relief depth")?;
        }
        Ok(out)
    }
}

/// Corner breaks std keeps on the definition's vertices (a fillet or chamfer of a sheet metal part's corner, batch
/// 017): each rounds or chamfers its wall's corner at the vertex, in the wall's plane.
fn break_corners(def: &Body, wall_attributes: &Attributes, wall: Shape) -> R<Shape> {
    let Some(wall_id) =
        sm_attribute(wall_attributes, "WALL").and_then(|a| a.get("attributeId")).and_then(Value::as_str)
    else {
        return Ok(wall);
    };
    let mut breaks: Vec<(Vec3, f64, bool)> = Vec::new();
    for v in &def.vertices {
        let Some(corner) = sm_attribute(&v.attributes, "CORNER") else { continue };
        let Some(list) = corner.get("cornerBreaks").and_then(Value::as_array) else { continue };
        for entry in list.iter() {
            let b = entry.get("value").unwrap_or(entry);
            if b.get("wallId").and_then(Value::as_str) != Some(wall_id) {
                continue;
            }
            let size = values::length(field(b, "range"), "corner break range")?;
            let chamfer = b.get("cornerBreakStyle").and_then(Value::as_str) == Some("CHAMFER");
            breaks.push((v.shape.vertex_point().map_err(occt_error)?, size, chamfer));
        }
    }
    if breaks.is_empty() {
        return Ok(wall);
    }
    let mut faces = Vec::new();
    for mut face in wall.sub_shapes(ShapeKind::Face) {
        for &(p, size, chamfer) in &breaks {
            let at: Vec<Shape> = face
                .sub_shapes(ShapeKind::Vertex)
                .into_iter()
                .filter(|v| v.vertex_point().is_ok_and(|q| norm(sub(q, p)) < 1e-7))
                .collect();
            if !at.is_empty() {
                face = occt::face_corners_2d(&face, &at, size, chamfer)
                    .map_err(|_| regen_error("SHEET_METAL_CORNER_BREAK_FAILED"))?;
            }
        }
        faces.push(face);
    }
    if faces.len() == 1 {
        return Ok(faces.remove(0));
    }
    occt::sew_faces(&faces, 1e-7).map_err(occt_error)
}
/// The bend reliefs a wall needs where bends cover only part of its edges: the rest of each such edge is cut back
/// as the bend is, and at each end of the bend a slot is cut, as wide as the width scale × the thickness, reaching
/// the depth scale × the bend's radius in from the edge to the centre of an obround's round end (a rectangle goes
/// half its width further; a tear cuts nothing more). Fitted to fixture batch 016. A sized rectangle reaches its
/// depth in from where the wall is cut back (batch 017); a sized obround is taken to reach as far.
/// Edges to cut back (with how far), and slots to cut out.
type Reliefs = (Vec<(Shape, f64)>, Vec<Shape>);

fn bend_reliefs(def: &Body, wall: &Shape, bends: &[Bend], relief: &BendRelief) -> R<Reliefs> {
    let mut stubs = Vec::new();
    let mut slots = Vec::new();
    let edges = wall.sub_shapes(ShapeKind::Edge);
    let n = wall.face_plane().map_err(occt_error)?.normal;
    for bend in bends {
        if !edges.iter().any(|e| e.is_same(&bend.edge)) {
            continue;
        }
        let Some((a, b)) = segment(&bend.edge) else { continue };
        let Some(e) = normalize(sub(b, a)) else { continue };
        let on_line = |p: Vec3| norm(sub(sub(p, a), scale(e, dot(sub(p, a), e)))) < 1e-9;
        let into = into_face(wall, scale(add(a, b), 0.5), e)?;
        for edge in &edges {
            if edge.is_same(&bend.edge) {
                continue;
            }
            let Some((c, d)) = segment(edge) else { continue };
            if !(on_line(c) && on_line(d)) {
                continue;
            }
            stubs.push((edge.clone(), bend.setback));
            // The end of the bend this stub carries on from, and the way out along it.
            let (end, out) =
                if norm(sub(c, a)) < 1e-9 || norm(sub(d, a)) < 1e-9 { (a, scale(e, -1.0)) } else { (b, e) };
            let relief = relief.at(def, end)?;
            if relief.style == "TEAR" {
                continue;
            }
            let width = relief.width_scale * relief.thickness;
            // In from the edge to an obround's centre, or to the end of a rectangle.
            let (depth, reach) = match relief.style.as_str() {
                "SIZED_RECTANGLE" => (0.0, bend.setback + relief.depth),
                "SIZED_OBROUND" => {
                    (bend.setback + relief.depth - width / 2.0, bend.setback + relief.depth - width / 2.0)
                }
                "RECTANGLE" => (0.0, relief.depth_scale * bend.radii.0 + width / 2.0),
                _ => (relief.depth_scale * bend.radii.0, relief.depth_scale * bend.radii.0),
            };
            let v_axis = cross(n, out);
            let far = norm(sub(b, a)) + 1.0;
            let (v0, v1) = if dot(v_axis, into) > 0.0 { (-far, reach) } else { (-reach, far) };
            slots.push(
                occt::plane_face(occt::Frame { origin: end, normal: n, x: out }, 0.0, width, v0, v1)
                    .map_err(occt_error)?,
            );
            if relief.style == "OBROUND" || relief.style == "SIZED_OBROUND" {
                let centre = add(add(end, scale(out, width / 2.0)), scale(into, depth));
                let circle = occt::circle_edge(occt::Frame { origin: centre, normal: n, x: out }, width / 2.0)
                    .map_err(occt_error)?;
                slots.push(occt::fill_edges(&[circle]).map_err(occt_error)?);
            }
        }
    }
    Ok((stubs, slots))
}

#[allow(clippy::too_many_arguments)]
fn build_part(
    def: &Body,
    bends: &[Bend],
    rips: &[usize],
    front: f64,
    back: f64,
    clearance: f64,
    relief: &BendRelief,
    corners: &[CornerRelief],
    tools: &[Vec<Shape>],
    forms: &[(Vec<Shape>, Vec<Shape>)],
) -> R<Shape> {
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
            } else if let Some(&r) = rips.iter().find(|&&r| def.edges[r].shape.is_same(&edge)) {
                let walls = faces_of_edge(&edge);
                let Some(other) = walls.iter().find(|f| !f.is_same(&wall.shape)) else { continue };
                // A butt joint runs one wall on over the other's edge (the first of the two in the sheet's order
                // for direction 1, the second for direction 2: fits batch 017's one case); the other stops short
                // as at an edge joint.
                let style = sm_attribute(&def.edges[r].attributes, "JOINT")
                    .and_then(|j| attribute_value(j, "jointStyle"))
                    .and_then(Value::as_str)
                    .unwrap_or("EDGE");
                let first = walls.first().is_some_and(|f| f.is_same(&wall.shape));
                let overlaps = (style == "BUTT" && first) || (style == "BUTT2" && !first);
                let setback = if overlaps {
                    overlap_setback(&wall.shape, other, &edge, front, back)?
                } else {
                    rip_setback(&wall.shape, other, &edge, front, back, clearance)?
                };
                edges.push(edge);
                distances.push(-setback);
            }
        }
        // A rolled wall, or a bend the model already rounds (std's bend wraps one in), is thickened as it is.
        let planar = wall.shape.surface_type().ok() == Some(occt::SurfaceType::Plane);
        let (stubs, mut slots) =
            if planar { bend_reliefs(def, &wall.shape, bends, relief)? } else { (Vec::new(), Vec::new()) };
        // Corner reliefs: cut as they are from the base wall, and carried into the walls beyond the bends.
        for corner in corners {
            if wall.shape.is_same(&corner.base) {
                slots.push(corner.hole.clone());
                continue;
            }
            for &k in &[corner.bends.0, corner.bends.1] {
                if !corner.closed.is_empty() {
                    continue;
                }
                let bend = &bends[k];
                let far_is_first = bend.walls.0.is_same(&wall.shape);
                if !(far_is_first || bend.walls.1.is_same(&wall.shape)) {
                    continue;
                }
                let back = unfolding(bend, !far_is_first).inverse();
                slots.push(occt::transform(&corner.hole, &back.transform()).map_err(occt_error)?.shape());
            }
        }
        for (edge, setback) in stubs {
            if !edges.iter().any(|e| e.is_same(&edge)) {
                edges.push(edge);
                distances.push(-setback);
            }
        }
        // Edges run on (a butt joint's overlapping wall) are extended first; the rest are cut back.
        let mut base = wall.shape.clone();
        if planar && distances.iter().any(|&d| d > 0.0) {
            let (grow, grow_by): (Vec<Shape>, Vec<f64>) =
                edges.iter().zip(&distances).filter(|(_, d)| **d > 0.0).map(|(e, d)| (e.clone(), *d)).unzip();
            base = occt::extend_planar_face(&base, &grow, &grow_by).map_err(occt_error)?;
            let keep: Vec<(Shape, f64)> =
                edges.iter().zip(&distances).filter(|(_, d)| **d <= 0.0).map(|(e, d)| (e.clone(), *d)).collect();
            edges = keep.iter().map(|(e, _)| e.clone()).collect();
            distances = keep.iter().map(|(_, d)| *d).collect();
        }
        let mut cut = if (edges.is_empty() && slots.is_empty()) || !planar {
            base
        } else {
            cut_back(&base, &edges, &distances, &slots)?
        };
        if planar {
            cut = break_corners(def, &wall.attributes, cut)?;
        }
        for face in cut.sub_shapes(ShapeKind::Face) {
            let solid =
                occt::thicken(&face, front, back).map_err(|_| regen_error("SHEET_METAL_CANNOT_THICKEN"))?.shape();
            let mut solid = place(solid, w)?;
            if !tools[w].is_empty() {
                solid = occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(&solid), &tools[w], true)
                    .map_err(|_| regen_error("SHEET_METAL_REBUILD_ERROR"))?
                    .shape();
            }
            // Forms: their parts to remove taken away, then their parts to add joined on.
            let (remove, join) = &forms[w];
            if !remove.is_empty() {
                solid = occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(&solid), remove, true)
                    .map_err(|_| regen_error("SHEET_METAL_FORMED_REBUILD_FAILED"))?
                    .shape();
            }
            if !join.is_empty() {
                solid = occt::boolean(occt::BooleanKind::Union, std::slice::from_ref(&solid), join, true)
                    .map_err(|_| regen_error("SHEET_METAL_FORMED_REBUILD_FAILED"))?
                    .shape();
            }
            pieces.push(solid);
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
                        let (lo, hi) = reach.unwrap_or((f64::MAX, f64::MIN));
                        reach = Some((lo.min(ta.min(tb)), hi.max(ta.max(tb))));
                    }
                }
            }
            let (lo, hi) = reach.ok_or_else(|| regen_error("SHEET_METAL_REBUILD_ERROR"))?;
            span = (span.0.max(lo), span.1.min(hi));
        }
        let relieved: Vec<&CornerRelief> = corners
            .iter()
            .filter(|c| std::ptr::eq(&bends[c.bends.0], bend) || std::ptr::eq(&bends[c.bends.1], bend))
            .collect();
        if !relieved.is_empty() {
            // Out to the corner: the relief takes the corner away.
            if let Some((a, b)) = segment(&bend.edge) {
                let (ta, tb) = (dot(sub(a, bend.at), bend.axis_direction), dot(sub(b, bend.at), bend.axis_direction));
                span = (ta.min(tb), ta.max(tb));
            }
        }
        // No further than the bend's own edge (a wall's edge can run on past a bend that covers part of it).
        if let Some((a, b)) = segment(&bend.edge) {
            let (ta, tb) = (dot(sub(a, bend.at), bend.axis_direction), dot(sub(b, bend.at), bend.axis_direction));
            span = (span.0.max(ta.min(tb)), span.1.min(ta.max(tb)));
        }
        if span.1 - span.0 <= 1e-9 {
            continue;
        }
        if !relieved.is_empty() && !bend.unfolded {
            // Built flat, the reliefs cut from it, and wrapped round the bend at its neutral radius.
            let base = &relieved[0].base;
            let (toward, into, normal) = if bend.walls.0.is_same(base) {
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
            let index = bends.iter().position(|b| std::ptr::eq(b, bend)).expect("a bend of the model");
            let holes: Vec<Shape> = relieved
                .iter()
                .map(|c| match c.closed.iter().find(|(k, _)| *k == index) {
                    Some((_, region)) => region.clone(),
                    None => c.hole.clone(),
                })
                .collect();
            let cut = occt::boolean(occt::BooleanKind::Subtraction, &[strip], &holes, true)
                .map_err(|_| regen_error("SHEET_METAL_REBUILD_ERROR"))?
                .shape();
            let cylinder = occt::cylinder_face(
                occt::Frame { origin: bend.axis_origin, normal: bend.axis_direction, x: toward },
                bend.sheet_radius,
                1.0,
            )
            .map_err(occt_error)?;
            let turn = std::f64::consts::PI - dot(bend.into.0, bend.into.1).clamp(-1.0, 1.0).acos();
            let neutral = bend.flat_length / turn;
            let faces = cut.sub_shapes(ShapeKind::Face);
            if faces.is_empty() {
                continue;
            }
            let wrapped =
                occt::wrap_onto_cylinder(&faces, (start, away, normal), &cylinder, start, away, Some(neutral))
                    .map_err(|_| regen_error("SHEET_METAL_REBUILD_ERROR"))?;
            // The wrapped faces face away from the axis; the material lies in front of the definition's normal.
            let (along, against) = if dot(normal, toward) > 0.0 { (front, back) } else { (back, front) };
            for face in wrapped.shape().sub_shapes(ShapeKind::Face) {
                let solid = occt::thicken(&face, along, against)
                    .map_err(|_| regen_error("SHEET_METAL_CANNOT_THICKEN"))?
                    .shape();
                pieces.push(place(solid, wall_index(&bend.walls.0))?);
            }
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
    if pieces.is_empty() {
        return Err(regen_error("SHEET_METAL_REBUILD_ERROR"));
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

/// The info of the tool face a part's face lies on, if it lies on one: same kind of surface, a point inside it on
/// the tool face.
fn tool_face_info(tool_faces: &[(Shape, Rc<Info>)], face: &Shape) -> Option<Rc<Info>> {
    let p = face.face_inner_point().ok()?;
    let kind = face.surface_type().ok()?;
    tool_faces
        .iter()
        .find(|(t, _)| {
            t.surface_type().ok() == Some(kind)
                && occt::vertex(p).and_then(|v| v.distance(t)).is_ok_and(|d| d.distance < 1e-8)
        })
        .map(|(_, info)| info.clone())
}

/// `@updateSheetMetalGeometry`: rebuilds the parts of the models whose definition entities changed.
pub fn update(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    // The models of the entities changed, and of the associated changes (a joint make joint added, batch 017).
    let mut defs: Vec<u64> = Vec::new();
    let mut changed = query::evaluate(model, field(definition, "entities"))?;
    if !field(definition, "associatedChanges").is_undefined() {
        changed.extend(query::evaluate(model, field(definition, "associatedChanges"))?);
    }
    for r in changed {
        let body = &model.bodies[r.body];
        if body.is_sheet_metal_definition() && !defs.contains(&body.tid) {
            defs.push(body.tid);
        }
    }
    for tid in defs {
        let Some(index) = model.bodies.iter().position(|b| b.tid == tid) else { continue };
        if is_active(&model.bodies[index]) {
            cut_registered_tools(model, &id, index)?;
            place_form_connectors(model, &id, index)?;
            let index = model
                .bodies
                .iter()
                .position(|b| b.tid == tid)
                .ok_or_else(|| regen_error("SHEET_METAL_REBUILD_ERROR"))?;
            regenerate(model, &id, index)?;
        } else {
            release_parts(model, index);
        }
    }
    Ok(Value::UNDEFINED)
}

/// A form's placement mate connector ends up a sheet thickness from where it was placed, the way the material lies from
/// the definition sheet (batch 022: (20, 15, 2) on a plate's top goes to (20, 15, 0); a flipped form's at (20, 15, 0) to (20, 15, -2)),
/// as a new body made by the update: std then deletes what's left of the form's instance, but not it. Each moves once.
fn place_form_connectors(model: &mut Model, id: &FeatureId, index: usize) -> R<()> {
    let def = model.bodies[index].clone();
    let Some(model_attribute) = sm_attribute(&def.attributes, "MODEL").cloned() else { return Ok(()) };
    let side = |k: &str| -> R<f64> {
        attribute_value(&model_attribute, k)
            .map(|v| values::length(v, "thickness"))
            .transpose()
            .map(|t| t.unwrap_or(0.0))
    };
    let (front, back) = (side("frontThickness")?, side("backThickness")?);
    let thickness = front + back;
    for wall in &def.faces {
        let Ok(plane) = wall.shape.face_plane() else { continue };
        for form in wall_forms(&wall.attributes) {
            let Some(tid) = form.placement else { continue };
            if model.placed_forms.contains(&tid) {
                continue;
            }
            let Some(b) = model.bodies.iter().position(|body| body.tid == tid) else { continue };
            // Towards the material from the definition sheet.
            let toward_material = if front >= back { plane.normal } else { scale(plane.normal, -1.0) };
            let t = super::translation(scale(toward_material, thickness));
            let body = model.bodies[b].clone();
            let moved = occt::transform(&body.shape, &t).map_err(occt_error)?;
            let frame = body.frame.map(|f| super::transform_frame(&f, &t));
            model.remove_bodies(&[b]);
            let info = Info::new(id.clone());
            // It keeps its transient id, which the wall's form records (std's form pattern copies it by that id,
            // batch 033).
            let new = NewBody { frame, tid: Some(tid), ..NewBody::new(moved.shape(), body.body_type, info.clone()) };
            model.add_body(new, &Origins::default(), &info, &|_| None);
            Rc::make_mut(&mut model.placed_forms).push(tid);
        }
    }
    Ok(())
}

/// The bodies registered on the walls to cut them (`cuttingToolBodyIds`: holes, batch 017) are cut from the
/// definition sheet itself: std looks for the circular edges they leave on it, made by the update and tracked from
/// the tools, and the parts take the holes from it. A tool already cut (the sheet has edges on it) is left alone.
/// The new entities get association attributes, as std gives the entities it makes.
fn cut_registered_tools(model: &mut Model, id: &FeatureId, index: usize) -> R<()> {
    let def = model.bodies[index].clone();
    let model_attribute = sm_attribute(&def.attributes, "MODEL").cloned();
    let thickness_of = |key: &str| -> R<f64> {
        match model_attribute.as_ref().and_then(|m| attribute_value(m, key)) {
            Some(v) => values::length(v, "sheet metal thickness"),
            None => Ok(0.0),
        }
    };
    let (front, back) = (thickness_of("frontThickness")?, thickness_of("backThickness")?);
    let mut tool_bodies: Vec<usize> = Vec::new();
    // Each tool and its moved copies.
    let mut moved: Vec<(usize, Vec<Shape>)> = Vec::new();
    // Each tool's outline in the sheet: what of it goes through the material, from one face to the other (a
    // counterbore leaves only its hole, batch 018). The tool is moved so its sections at the wall's two faces
    // fall on the sheet, and the two copies' common part cut.
    let mut tools: Vec<Shape> = Vec::new();
    for wall in &def.faces {
        let Ok(plane) = wall.shape.face_plane() else { continue };
        for tid in wall_tool_tids(&wall.attributes) {
            let Some(b) = model.bodies.iter().position(|b| b.tid == tid) else { continue };
            if tool_bodies.contains(&b) {
                continue;
            }
            let tool = &model.bodies[b].shape;
            if def.edges.iter().any(|e| lies_on(&samples(&e.shape), tool)) {
                continue;
            }
            let shifted = |by: f64| -> R<Shape> {
                Ok(occt::transform(tool, &super::translation(scale(plane.normal, by))).map_err(occt_error)?.shape())
            };
            let inset = 1e-7;
            let (top, bottom) = (shifted(-(front - inset))?, shifted(back - inset)?);
            let through = occt::boolean(
                occt::BooleanKind::Intersection,
                std::slice::from_ref(&top),
                std::slice::from_ref(&bottom),
                false,
            )
            .map_err(|_| regen_error("SHEET_METAL_REBUILD_ERROR"))?
            .shape();
            tool_bodies.push(b);
            tools.push(through);
            moved.push((b, vec![top, bottom]));
        }
    }
    if tool_bodies.is_empty() {
        return Ok(());
    }
    let op = occt::boolean(occt::BooleanKind::Subtraction, std::slice::from_ref(&def.shape), &tools, false)
        .map_err(|_| regen_error("SHEET_METAL_REBUILD_ERROR"))?;
    let mut inputs: Vec<&Body> = vec![&def];
    let tool_rcs: Vec<Rc<Body>> = tool_bodies.iter().map(|&b| model.bodies[b].clone()).collect();
    inputs.extend(tool_rcs.iter().map(|b| &**b));
    let origins = Origins::of_operation(&op, &inputs);
    let shape = op.shape();
    let shape = shape.sub_shapes(ShapeKind::Shell).into_iter().next().unwrap_or(shape);
    // The tools' faces (and their moved copies' faces, in the same order), and what they came from: the cut's edges
    // are tracked back to the faces they lie on.
    let mut tool_faces: Vec<(Shape, Vec<crate::model::Tid>)> = Vec::new();
    for (b, copies) in &moved {
        let body = &model.bodies[*b];
        let tids: Vec<Vec<crate::model::Tid>> =
            body.faces.iter().map(|f| std::iter::once(f.tid).chain(f.ancestry.all.iter().copied()).collect()).collect();
        for shape in std::iter::once(&body.shape).chain(copies.iter()) {
            for (face, t) in shape.sub_shapes(ShapeKind::Face).into_iter().zip(&tids) {
                tool_faces.push((face, t.clone()));
            }
        }
    }
    model.remove_bodies(&[index]);
    let info = Info::new(id.clone());
    let new = model.add_body(NewBody::replacing(&def, shape), &origins, &info, &|_| None);
    for i in 0..model.bodies[new].edges.len() {
        let r = EntityRef { body: new, kind: EntityType::Edge, index: i };
        if !model.info(r).is_created_by(id) {
            continue;
        }
        let points = samples(&model.bodies[new].edges[i].shape);
        let from: Vec<crate::model::Tid> = tool_faces
            .iter()
            .filter(|(face, _)| lies_on(&points, face))
            .flat_map(|(_, tids)| tids.iter().copied())
            .collect();
        if !from.is_empty() {
            model.add_ancestors(r, &from);
        }
    }
    // Association attributes for what the cut made.
    let Some(tag) = fs_interp::types::find("SMAssociationAttribute") else { return Ok(()) };
    for kind in [EntityType::Face, EntityType::Edge, EntityType::Vertex] {
        for i in 0..model.bodies[new].entities(kind).len() {
            let entity = &model.bodies[new].entities(kind)[i];
            if association(&entity.attributes).is_some() {
                continue;
            }
            let name = format!("{{ queryType : TRANSIENT , transientId : {} }}", crate::query::tid_string(entity.tid));
            let mut attribute = values::map(vec![("attributeId", Value::string(name))]);
            attribute.tag = Some(tag);
            model.update_attributes(EntityRef { body: new, kind, index: i }, |attributes| {
                attributes.unnamed.push(attribute)
            });
        }
    }
    Ok(())
}

/// `@opExtendSheetBody` in the form sheet metal uses (`extendMethod` EXTEND_TO_SURFACE, `limitEntity` a plane, at
/// `offset` from it): free straight edges of planar faces moved within their faces' planes up to the plane, and
/// the faces sewn where they then meet (make joint, batch 017). Faces keep their identities; an edge two faces now
/// share is new, tracked back to the edges it joins.
pub fn extend_to_plane(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let limit = field(definition, "limitEntity").clone();
    extend_to_plane_with(model, id, definition, &limit)
}

/// As [`extend_to_plane`], up to the plane `limit` gives (a plane, a planar face, or a body of one).
pub fn extend_to_plane_with(model: &mut Model, id: FeatureId, definition: &Value, limit: &Value) -> R<Value> {
    let plane = if limit.get("normal").is_some() {
        values::plane(limit, "opExtendSheetBody limitEntity")?
    } else {
        let found = query::evaluate(model, limit)?;
        let r = found.first().ok_or_else(|| regen_error("EXTEND_SHEET_BODY_FAILED"))?;
        let face = match r.kind {
            EntityType::Body => model.bodies[r.body].faces.first().map(|f| f.shape.clone()),
            _ => Some(model.shape(*r).clone()),
        }
        .ok_or_else(|| regen_error("EXTEND_SHEET_BODY_FAILED"))?;
        face.face_plane().map_err(|_| error("@opExtendSheetBody: only planar limits are supported yet"))?
    };
    let offset = match field(definition, "offset") {
        v if v.is_undefined() => 0.0,
        v => values::length(v, "opExtendSheetBody offset")?,
    };
    let failed = || regen_error("EXTEND_SHEET_BODY_FAILED");
    for (b, edges) in super::by_body(model, field(definition, "entities"), EntityType::Edge)? {
        let old = model.bodies[b].clone();
        let mut faces: Vec<Shape> = old.faces.iter().map(|f| f.shape.clone()).collect();
        // Each moved edge and how far it moved.
        let mut moved: Vec<(Shape, Vec3)> = Vec::new();
        for (i, face) in old.faces.iter().enumerate() {
            let own = face.shape.sub_shapes(ShapeKind::Edge);
            let selected: Vec<Shape> = edges.iter().filter(|e| own.iter().any(|x| x.is_same(e))).cloned().collect();
            if selected.is_empty() {
                continue;
            }
            let mut distances = Vec::new();
            for e in &selected {
                let (a, z) =
                    segment(e).ok_or_else(|| error("@opExtendSheetBody: only straight edges are supported yet"))?;
                let m = scale(add(a, z), 0.5);
                let out = scale(into_face(&face.shape, m, sub(z, a))?, -1.0);
                let rate = dot(out, plane.normal);
                if rate.abs() < 1e-9 || dot(normalize(sub(z, a)).ok_or_else(failed)?, plane.normal).abs() > 1e-9 {
                    return Err(failed());
                }
                let s = (offset - dot(sub(m, plane.origin), plane.normal)) / rate;
                distances.push(s);
                moved.push((e.clone(), scale(out, s)));
            }
            faces[i] = occt::extend_planar_face(&face.shape, &selected, &distances).map_err(|_| failed())?;
        }
        let shell = occt::sew_faces(&faces, 1e-7).map_err(occt_error)?;
        let shift_of = |shape: &Shape| moved.iter().find(|(e, _)| e.is_same(shape)).map(|(_, v)| *v);
        let shifted = |points: Vec<Vec3>, v: Option<Vec3>| -> Vec<Vec3> {
            points.into_iter().map(|p| v.map_or(p, |v| add(p, v))).collect()
        };
        let mut origins = Origins::default();
        for (i, face) in old.faces.iter().enumerate() {
            let target = &faces[i];
            if let Some(image) = shell
                .sub_shapes(ShapeKind::Face)
                .into_iter()
                .find(|f| lies_on(&samples(target), f) && lies_on(&samples(f), target))
            {
                origins.same(&image, face);
            }
        }
        for image in shell.sub_shapes(ShapeKind::Edge) {
            let image_points = samples(&image);
            // The old edges it is (moved, if they were), and those it runs along (made longer).
            let mut exact = Vec::new();
            let mut along = Vec::new();
            for edge in &old.edges {
                let v = shift_of(&edge.shape);
                let points = shifted(samples(&edge.shape), v);
                if !lies_on(&points, &image) {
                    continue;
                }
                let back: Vec<Vec3> = image_points.iter().map(|&p| v.map_or(p, |v| sub(p, v))).collect();
                if lies_on(&back, &edge.shape) { exact.push((edge, v.is_some())) } else { along.push(edge) }
            }
            match (exact.as_slice(), along.is_empty()) {
                ([(edge, false)], true) => origins.same(&image, edge),
                _ => {
                    for (edge, _) in &exact {
                        origins.inherit(&image, edge);
                    }
                    for edge in &along {
                        origins.inherit(&image, edge);
                    }
                }
            }
        }
        for image in shell.sub_shapes(ShapeKind::Vertex) {
            let Ok(p) = image.vertex_point() else { continue };
            for vertex in &old.vertices {
                let Ok(q) = vertex.shape.vertex_point() else { continue };
                if norm(sub(p, q)) < 1e-7 {
                    origins.same(&image, vertex);
                }
            }
        }
        model.remove_bodies(&[b]);
        let info = Info::new(id.clone());
        model.add_body(NewBody::replacing(&old, shell), &origins, &info, &|_| None);
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
    // A circular fillet, an equal-offset chamfer, a two-offset chamfer (the first width along the edge leaving the
    // corner as the wall's loop runs, batch 020), or a conic set back by the radius.
    enum Break {
        Round(f64),
        Chamfer(f64, f64),
        Conic(f64, f64),
    }
    let length = |key: &str| values::length(field(definition, key), key);
    let corner_break = match (field(definition, "crossSection").as_str(), field(definition, "chamferType").as_str()) {
        (Some("CHAMFER"), None | Some("EQUAL_OFFSETS")) => {
            let w = length("width")?;
            Break::Chamfer(w, w)
        }
        (Some("CHAMFER"), Some("TWO_OFFSETS")) => {
            let (w1, w2) = (length("width1")?, length("width2")?);
            // Kept as (along the edge coming in, along the edge leaving).
            if values::opt_bool(definition, "oppositeDirection")?.unwrap_or(false) {
                Break::Chamfer(w1, w2)
            } else {
                Break::Chamfer(w2, w1)
            }
        }
        (Some("CHAMFER"), Some(other)) => {
            return Err(error(format!("@sheetMetalApplyInFlat: {other} chamfers aren't supported yet")));
        }
        (Some("CONIC"), _) => {
            let rho = field(definition, "rho").as_number().ok_or_else(|| error("@sheetMetalApplyInFlat: no rho"))?;
            Break::Conic(length("radius")?, rho)
        }
        (None | Some("CIRCULAR"), _) => Break::Round(length("radius")?),
        (Some(other), _) => return Err(error(format!("@sheetMetalApplyInFlat: {other} fillets aren't supported yet"))),
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
            let failed = || regen_error("SHEET_METAL_CORNER_BREAK_FAILED");
            faces[i] = match corner_break {
                Break::Round(r) => occt::face_corners_2d(&face.shape, &at, r, false).map_err(|_| failed())?,
                Break::Chamfer(d1, d2) if (d1 - d2).abs() < 1e-12 => {
                    occt::face_corners_2d(&face.shape, &at, d1, true).map_err(|_| failed())?
                }
                Break::Chamfer(d1, d2) => {
                    let mut f = face.shape.clone();
                    for v in &at {
                        let corner =
                            f.sub_shapes(ShapeKind::Vertex).into_iter().find(|x| x.is_same(v)).ok_or_else(failed)?;
                        f = occt::face_corner_chamfer_2d(&f, &corner, d1, d2).map_err(|_| failed())?;
                    }
                    f
                }
                Break::Conic(r, rho) => {
                    let mut f = face.shape.clone();
                    for v in &at {
                        let p = v.vertex_point().map_err(occt_error)?;
                        let corner = f
                            .sub_shapes(ShapeKind::Vertex)
                            .into_iter()
                            .find(|x| x.vertex_point().is_ok_and(|q| norm(sub(q, p)) < 1e-9))
                            .ok_or_else(failed)?;
                        f = occt::face_corner_conic_2d(&f, &corner, r, rho).map_err(|_| failed())?;
                    }
                    f
                }
            };
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
