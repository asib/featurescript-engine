//! Evaluating std's queries: maps with a `queryType` that `@evaluateQuery` resolves against the model.

use crate::model::{EntityRef, EntityType, Model, Tid};
use crate::values::{self, field};
use crate::{GeomBuiltins, model_mut, tags, topology};
use fs_interp::Value;
use fs_interp::builtins::arg;
use fs_interp::eval::{R, error};
use occt::ShapeKind;
use std::collections::BTreeSet;

pub fn register(m: &mut GeomBuiltins) {
    m.insert("evaluateQuery", |interp, args| {
        let tags = tags(interp)?;
        let query = arg(&args, 1, "evaluateQuery")?.get("query").cloned().unwrap_or(Value::UNDEFINED);
        model_mut(&args, "evaluateQuery", |model| {
            let found = evaluate(model, &query)?;
            Ok(Value::array(found.into_iter().map(|r| tags.transient_query(&tid_string(model.tid(r)))).collect()))
        })
    });
    m.insert("evaluateQueryCount", |_, args| {
        let query = arg(&args, 1, "evaluateQueryCount")?.get("query").cloned().unwrap_or(Value::UNDEFINED);
        model_mut(&args, "evaluateQueryCount", |model| Ok(Value::number(evaluate(model, &query)?.len() as f64)))
    });
    m.insert("isQueryEmpty", |_, args| {
        let query = arg(&args, 1, "isQueryEmpty")?.get("query").cloned().unwrap_or(Value::UNDEFINED);
        model_mut(&args, "isQueryEmpty", |model| Ok(Value::bool(evaluate(model, &query)?.is_empty())))
    });
    m.insert("transientIdToString", |_, args| Ok(arg(&args, 0, "transientIdToString")?.clone().with_tag(None)));
}

/// How close (metres) an entity must come to a point to contain it.
const CONTAINS_TOLERANCE: f64 = 1e-8;

pub fn tid_string(tid: Tid) -> String {
    format!("T{tid}")
}

fn parse_tid(s: &str) -> Option<Tid> {
    s.strip_prefix('T')?.parse().ok()
}

fn query_type(query: &Value) -> R<&str> {
    query
        .get("queryType")
        .and_then(Value::as_str)
        .ok_or_else(|| error(format!("expected a query, not {}", query.type_description())))
}

fn entity_type(v: &Value) -> R<Option<EntityType>> {
    if v.is_undefined() {
        return Ok(None);
    }
    let name = v.as_str().ok_or_else(|| error("entityType must be an EntityType"))?;
    EntityType::from_name(name).map(Some).ok_or_else(|| error(format!("unknown EntityType {name}")))
}

/// Resolves a query to bodies and entities, in model order without duplicates.
pub fn evaluate(model: &Model, query: &Value) -> R<Vec<EntityRef>> {
    let found = resolve(model, query)?;
    let mut seen = BTreeSet::new();
    Ok(found.into_iter().filter(|r| seen.insert(*r)).collect())
}

fn sub(model: &Model, query: &Value, key: &str) -> R<Vec<EntityRef>> {
    evaluate(model, field(query, key))
}

fn filter_type(found: Vec<EntityRef>, kind: Option<EntityType>) -> Vec<EntityRef> {
    match kind {
        Some(kind) => found.into_iter().filter(|r| r.kind == kind).collect(),
        None => found,
    }
}

fn yes_no(query: &Value, key: &str) -> R<bool> {
    match field(query, key).as_str() {
        Some("YES") => Ok(true),
        Some("NO") => Ok(false),
        other => Err(error(format!("{key}: expected YES or NO, not {other:?}"))),
    }
}

fn resolve(model: &Model, query: &Value) -> R<Vec<EntityRef>> {
    if query.is_undefined() {
        return Err(error("evaluateQuery: no query"));
    }
    let kind = query_type(query)?;
    Ok(match kind {
        "NOTHING" => Vec::new(),
        // qBodyType folds its filter into an EVERYTHING query as `bodyType`.
        "EVERYTHING" => {
            let found = model.everything(entity_type(field(query, "entityType"))?);
            match field(query, "bodyType") {
                v if v.is_undefined() => found,
                types => {
                    let wanted: Vec<&str> = match types.as_array() {
                        Some(items) => items.iter().filter_map(Value::as_str).collect(),
                        None => types.as_str().into_iter().collect(),
                    };
                    found.into_iter().filter(|r| wanted.contains(&model.body(*r).body_type.name())).collect()
                }
            }
        }
        "TRANSIENT" => {
            let id = field(query, "transientId").as_str().and_then(parse_tid);
            id.and_then(|tid| model.find_tid(tid)).into_iter().collect()
        }
        "CREATED_BY" => {
            let feature = values::feature_id(field(query, "featureId"), "qCreatedBy")?;
            let kind = entity_type(field(query, "entityType"))?;
            visible(model, model.everything(kind))
                .into_iter()
                .filter(|r| model.info(*r).is_created_by(&feature))
                .collect()
        }
        "SKETCH_REGION" => {
            let feature = values::feature_id(field(query, "featureId"), "qSketchRegion")?;
            let filter_inner = field(query, "filterInnerLoops").as_bool().unwrap_or(false);
            sketch_regions(model, &feature, filter_inner)
        }
        "UNION" => {
            let subqueries = field(query, "subqueries").as_array().ok_or_else(|| error("qUnion: expected an array"))?;
            let mut out = Vec::new();
            for q in subqueries.iter() {
                out.extend(resolve(model, q)?);
            }
            out
        }
        "INTERSECTION" => {
            let subqueries =
                field(query, "subqueries").as_array().ok_or_else(|| error("qIntersection: expected an array"))?;
            let mut out: Option<Vec<EntityRef>> = None;
            for q in subqueries.iter() {
                let found: BTreeSet<EntityRef> = resolve(model, q)?.into_iter().collect();
                out = Some(match out {
                    None => found.into_iter().collect(),
                    Some(previous) => previous.into_iter().filter(|r| found.contains(r)).collect(),
                });
            }
            out.unwrap_or_default()
        }
        "SUBTRACTION" => {
            let remove: BTreeSet<EntityRef> = sub(model, query, "query2")?.into_iter().collect();
            sub(model, query, "query1")?.into_iter().filter(|r| !remove.contains(r)).collect()
        }
        "ENTITY_FILTER" => filter_type(sub(model, query, "subquery")?, entity_type(field(query, "entityType"))?),
        "NTH_ELEMENT" => {
            let n = field(query, "n").as_number().ok_or_else(|| error("qNthElement: n must be a number"))?;
            let found = sub(model, query, "subquery")?;
            let index = if n < 0.0 { found.len() as f64 + n } else { n };
            if index >= 0.0 && index.fract() == 0.0 {
                found.get(index as usize).copied().into_iter().collect()
            } else {
                Vec::new()
            }
        }
        "BODY_TYPE" => {
            let types = field(query, "bodyType");
            let wanted: Vec<&str> = match types.as_array() {
                Some(items) => items.iter().filter_map(Value::as_str).collect(),
                None => types.as_str().into_iter().collect(),
            };
            sub(model, query, "subquery")?
                .into_iter()
                .filter(|r| wanted.contains(&model.body(*r).body_type.name()))
                .collect()
        }
        "CONSTRUCTION_FILTER" => {
            let construction = yes_no(query, "constructionFilter")?;
            sub(model, query, "subquery")?.into_iter().filter(|r| model.body(*r).construction == construction).collect()
        }
        "SKETCH_OBJECT_FILTER" => {
            let sketch = yes_no(query, "sketchObjectFilter")?;
            sub(model, query, "subquery")?
                .into_iter()
                .filter(|r| model.body(*r).sketch_plane.is_some() == sketch)
                .collect()
        }
        // Sheet metal isn't modelled, so nothing is flattened, formed or active sheet metal.
        // No sheet metal, so nothing is attached to any.
        "PARTS_ATTACHED_TO" => Vec::new(),
        "SM_FLAT_FILTER" => keep_if(sub(model, query, "subquery")?, !yes_no(query, "flatFilter")?),
        "SM_FORM_FILTER" => keep_if(sub(model, query, "subquery")?, !yes_no(query, "formFilter")?),
        "ACTIVE_SM_FILTER" => keep_if(sub(model, query, "subquery")?, !yes_no(query, "activeSheetMetal")?),
        // No sheet metal: nothing is a flat's counterpart or a sheet metal definition entity.
        "SM_APPLICATION_TYPE_FILTER" | "CORRESPONDING_IN_FLAT" | "SM_DEFINITION_ENTITY_FILTER" => Vec::new(),
        // Entities of bodies a closed composite part consumes (or not).
        "CONSUMED" => {
            let consumed = field(query, "consumed").as_str() == Some("YES");
            sub(model, query, "subquery")?
                .into_iter()
                .filter(|r| {
                    let is = model
                        .composites_containing(r.body)
                        .iter()
                        .any(|&c| model.bodies[c].composite.as_ref().is_some_and(|(_, closed)| *closed));
                    is == consumed
                })
                .collect()
        }
        // (Std spells it this way.)
        "COMPOSITE_PART_TYPE_FITLER" => {
            let closed = field(query, "compositePartType").as_str() == Some("CLOSED");
            sub(model, query, "subquery")?
                .into_iter()
                .filter(|r| {
                    r.kind == EntityType::Body
                        && model.bodies[r.body].composite.as_ref().is_some_and(|(_, is_closed)| *is_closed == closed)
                })
                .collect()
        }
        // A named entity, or what it became: entities carrying on its identity.
        "NAMED" => {
            let name = field(query, "name").as_str().unwrap_or_default();
            let tids: Vec<Tid> = model.names.iter().filter(|(n, _)| &**n == name).map(|(_, t)| *t).collect();
            let mut out = Vec::new();
            for (b, body) in model.bodies.iter().enumerate() {
                if tids.contains(&body.tid) {
                    out.push(EntityRef::body(b));
                }
                for kind in [EntityType::Face, EntityType::Edge, EntityType::Vertex] {
                    for (index, e) in body.entities(kind).iter().enumerate() {
                        if tids.contains(&e.tid) || e.ancestry.identity.iter().any(|t| tids.contains(t)) {
                            out.push(EntityRef { body: b, kind, index });
                        }
                    }
                }
            }
            out
        }
        "CONVEX_CONNECTED_FACES" => crate::selection::connected_faces(model, &sub(model, query, "subquery")?, "CONVEX"),
        "CONCAVE_CONNECTED_FACES" => {
            crate::selection::connected_faces(model, &sub(model, query, "subquery")?, "CONCAVE")
        }
        "HOLE_FACES" => crate::selection::hole_faces(model, &sub(model, query, "subquery")?),
        "FILLET_FACES" => {
            let compare = field(query, "compareType").as_str().unwrap_or("EQUAL");
            crate::selection::fillet_faces(model, &sub(model, query, "subquery")?, compare)
        }
        "LOOP_EDGES" => crate::selection::loop_edges(model, &sub(model, query, "subquery")?),
        "PARALLEL_EDGES" => {
            if !field(query, "subquery").is_undefined() {
                let (directions, all) = crate::selection::edge_directions(model, &sub(model, query, "subquery")?);
                crate::selection::parallel_edges(model, &all, &directions)
            } else {
                let candidates = sub(model, query, "queryToFilter")?;
                let directions = match field(query, "direction") {
                    v if v.is_undefined() => crate::selection::edge_directions(model, &sub(model, query, "edges")?).0,
                    v => vec![values::vec3(v, "qParallelEdges direction")?],
                };
                crate::selection::parallel_edges(model, &candidates, &directions)
            }
        }
        "LOOP_BOUNDED_FACES" => crate::selection::loop_bounded_faces(model, &sub(model, query, "subquery")?),
        "FACE_OR_EDGE_BOUNDED_FACES" => crate::selection::bounded_faces(model, &sub(model, query, "subquery")?),
        "PATTERN" => crate::selection::matching(model, &sub(model, query, "subquery")?),
        "PLANE_NORMAL" => {
            let normal = values::vec3(field(query, "normal"), "qParallelPlanes normal")?;
            let antiparallel = field(query, "allowAntiparallel").as_bool() != Some(false);
            crate::selection::planes_with_normal(model, &sub(model, query, "subquery")?, normal, antiparallel)
        }
        "FACE_PARALLEL_DIRECTION" => {
            let direction = values::vec3(field(query, "direction"), "qFacesParallelToDirection direction")?;
            crate::selection::faces_parallel_to(model, &sub(model, query, "subquery")?, direction)
        }
        // The mate connectors owned by some parts.
        "MATE_CONNECTOR" => {
            let owners: Vec<Tid> = sub(model, query, "subquery")?.iter().map(|r| model.bodies[r.body].tid).collect();
            (0..model.bodies.len())
                .filter(|&b| model.bodies[b].owner.is_some_and(|o| owners.contains(&o)))
                .map(EntityRef::body)
                .collect()
        }
        "PLANE_PARALLEL_DIRECTION" => {
            let direction = values::vec3(field(query, "direction"), "qPlanesParallelToDirection direction")?;
            crate::selection::planes_parallel_to(model, &sub(model, query, "subquery")?, direction)
        }
        "INTERSECTS_PLANE" => {
            let plane = values::plane(field(query, "plane"), "qIntersectsPlane plane")?;
            crate::selection::touching_plane(model, &sub(model, query, "subquery")?, plane)
        }
        "IN_FRONT_OF_PLANE" => {
            let plane = values::plane(field(query, "plane"), "qInFrontOfPlane plane")?;
            crate::selection::in_front_of(model, &sub(model, query, "subquery")?, plane)?
        }
        "EDGE_CONVEXITY_FILTER" => {
            let wanted = field(query, "convexityType").as_str().unwrap_or("CONVEX");
            sub(model, query, "subquery")?
                .into_iter()
                .filter(|r| r.kind == EntityType::Edge && crate::selection::convexity(model, *r, false) == Some(wanted))
                .collect()
        }
        // The faces a split put in front of (or behind) its tool.
        "SPLIT" => {
            let feature = values::feature_id(field(query, "operationId"), "qSplitBy operationId")?;
            let back = field(query, "isFromBackBody").as_bool() == Some(true);
            let kind = entity_type(field(query, "entityType"))?;
            let tids: Vec<Tid> = model
                .splits
                .iter()
                .filter(|(f, _, b)| f.starts_with(&feature) && *b == back)
                .map(|(_, t, _)| *t)
                .collect();
            filter_type(tids.iter().filter_map(|t| model.find_tid(*t)).collect(), kind)
        }
        // Edges by how many faces they join: none (a wire's), one (a sheet's boundary), or two.
        "EDGE_TOPOLOGY_FILTER" => {
            let wanted = field(query, "edgeTopologyType").as_str().unwrap_or("TWO_SIDED");
            sub(model, query, "subquery")?
                .into_iter()
                .filter(|r| r.kind == EntityType::Edge)
                .filter(|r| {
                    let edge = model.shape(*r);
                    let faces = model.bodies[r.body]
                        .faces
                        .iter()
                        .filter(|f| f.shape.sub_shapes(occt::ShapeKind::Edge).iter().any(|e| e.is_same(edge)))
                        .count();
                    match wanted {
                        "WIRE" => faces == 0,
                        "ONE_SIDED" | "LAMINAR" => faces == 1,
                        _ => faces >= 2,
                    }
                })
                .collect()
        }
        "ATTRIBUTE_FILTER" => {
            let found = match field(query, "subquery") {
                v if v.is_undefined() => model.everything(None),
                v => resolve(model, v)?,
            };
            crate::attributes::filter(model, found, &crate::attributes::Wanted::of(query))
        }
        // The faces and profile edges `@opHole` made, by the name it gave them and the hole they belong to.
        "OP_HOLE_FACE" | "OP_HOLE_PROFILE" => {
            let feature = values::feature_id(field(query, "featureId"), "hole query featureId")?;
            let name = field(query, "name").as_str();
            let identity = match field(query, "identity") {
                v if v.is_undefined() => None,
                v => Some(resolve(model, v)?.first().map(|r| model.tid(*r))),
            };
            let kind = if kind == "OP_HOLE_FACE" { EntityType::Face } else { EntityType::Edge };
            model
                .everything(Some(kind))
                .into_iter()
                .filter(|r| {
                    let info = model.info(*r);
                    info.is_created_by(&feature)
                        && info.label.is_some()
                        && name.is_none_or(|n| info.label.as_deref() == Some(n))
                        && identity.is_none_or(|i| i.is_some() && info.identity == i)
                })
                .collect()
        }
        "TRACKING" => tracking(model, query)?,
        // Meshes aren't modelled, so nothing is mesh geometry.
        "MESH_GEOMETRY_FILTER" => keep_if(sub(model, query, "subquery")?, !yes_no(query, "meshGeometryFilter")?),
        "CONTAINS_POINT" => {
            let point = values::vec3(field(query, "point"), "qContainsPoint point")?;
            let vertex = occt::vertex(point).map_err(crate::occt_error)?;
            sub(model, query, "subquery")?
                .into_iter()
                .filter(|r| model.shape(*r).distance(&vertex).is_ok_and(|d| d.distance <= CONTAINS_TOLERANCE))
                .collect()
        }
        // A sketch entity's edges, vertices or bodies. The operation id is the sketch's id, perhaps with "wireOp"
        // (its edges) appended.
        "SKETCH_ENTITY" => {
            let mut feature = values::feature_id(field(query, "operationId"), "sketch entity query")?.to_vec();
            if feature.last().is_some_and(|last| matches!(&**last, "wireOp" | "imprint")) {
                feature.pop();
            }
            let wanted = field(query, "sketchEntityId").as_str().unwrap_or_default();
            model
                .everything(entity_type(field(query, "entityType"))?)
                .into_iter()
                .filter(|r| {
                    let info = model.info(*r);
                    info.is_created_by(&feature) && info.sketch_entity.as_deref() == Some(wanted)
                })
                .collect()
        }
        // A sketch's imprint: the region faces its sketch made. The query's operation id is the sketch's id plus
        // "imprint".
        "IMPRINT" => {
            let mut feature = values::feature_id(field(query, "operationId"), "imprint query")?.to_vec();
            if feature.last().is_some_and(|last| &**last == "imprint") {
                feature.pop();
            }
            match field(query, "derivedFrom") {
                v if v.is_undefined() => {
                    filter_type(sketch_regions(model, &feature, false), entity_type(field(query, "entityType"))?)
                }
                // The regions' edges and vertices lying on given sketch entities (std's sketch entity tracking).
                from => filter_type(
                    imprints_of(model, &feature, &evaluate(model, from)?),
                    entity_type(field(query, "entityType"))?,
                ),
            }
        }
        "VERTEX_ADJACENT" | "EDGE_ADJACENT" => {
            let via = if kind == "VERTEX_ADJACENT" { EntityType::Vertex } else { EntityType::Edge };
            let seeds = sub(model, query, "query")?;
            topology::adjacent(model, &seeds, via, entity_type(field(query, "entityType"))?)
        }
        "EDGE_VERTEX" => {
            let at_start = field(query, "atStart").as_bool().unwrap_or(true);
            topology::edge_vertices(model, &sub(model, query, "query")?, at_start)
        }
        "TANGENT_CONNECTED_EDGES" => topology::tangent_connected_edges(model, &sub(model, query, "subquery")?),
        "TANGENT_CONNECTED_FACES" => {
            let tolerance = field(query, "angleTolerance").as_number().unwrap_or(topology::TANGENT_TOLERANCE);
            topology::tangent_connected_faces(model, &sub(model, query, "subquery")?, tolerance)
        }
        "CLOSEST_TO" => {
            let point = values::vec3(field(query, "point"), "qClosestTo point")?;
            topology::closest_to(model, &sub(model, query, "subquery")?, point)
        }
        "FARTHEST_ALONG" => {
            let direction = values::vec3(field(query, "direction"), "qFarthestAlong direction")?;
            topology::farthest_along(model, &sub(model, query, "subquery")?, direction)
        }
        "LARGEST" | "SMALLEST" => topology::by_size(model, &sub(model, query, "subquery")?, kind == "LARGEST"),
        "DEPENDENCY" => {
            let mut out = Vec::new();
            for r in sub(model, query, "subquery")? {
                if let Some(tids) = &model.info(r).dependency {
                    out.extend(tids.iter().filter_map(|tid| model.find_tid(*tid)));
                }
            }
            out
        }
        // What wire edges were made from, followed back to edges of sheets (laminar edges). Sketch edges come from
        // nothing, so they have none.
        "LAMINAR_DEPENDENCY" => {
            let mut out = Vec::new();
            for r in sub(model, query, "subquery")? {
                let mut current = vec![r];
                for _ in 0..8 {
                    let next: Vec<EntityRef> = current
                        .iter()
                        .filter_map(|c| model.info(*c).dependency.clone())
                        .flat_map(|tids| tids.iter().filter_map(|tid| model.find_tid(*tid)).collect::<Vec<_>>())
                        .collect();
                    if next.is_empty() {
                        break;
                    }
                    let laminar: Vec<EntityRef> = next
                        .iter()
                        .copied()
                        .filter(|e| {
                            e.kind == EntityType::Edge && model.body(*e).body_type == crate::model::BodyType::Sheet
                        })
                        .collect();
                    if !laminar.is_empty() {
                        out.extend(laminar);
                        break;
                    }
                    current = next;
                }
            }
            out
        }
        "COINCIDES_WITH_PLANE" => {
            let plane = values::plane(field(query, "plane"), "qCoincidesWithPlane plane")?;
            sub(model, query, "subquery")?
                .into_iter()
                .filter(|r| {
                    model
                        .shape(*r)
                        .bounding_box_in(plane, true)
                        .is_ok_and(|b| b.min.z.abs() < 1e-8 && b.max.z.abs() < 1e-8)
                })
                .collect()
        }
        // What an extrude or sweep swept from its profile's edges (faces) or vertices (edges): what it created
        // other than its caps.
        "SWEPT_FACE" | "SWEPT_EDGE" => {
            let feature = values::feature_id(field(query, "operationId"), "swept query")?;
            let wanted = if kind == "SWEPT_FACE" { EntityType::Face } else { EntityType::Edge };
            model
                .everything(Some(wanted))
                .into_iter()
                .filter(|r| {
                    let info = model.info(*r);
                    info.is_created_by(&feature) && info.cap.is_none()
                })
                .collect()
        }
        // The bodies in composite parts, and the composite parts containing bodies.
        "CONTAINED_IN_COMPOSITE" => {
            let mut out = Vec::new();
            for r in sub(model, query, "compositeParts")? {
                if let Some((members, _)) = &model.bodies[r.body].composite {
                    out.extend(model.composite_members(members).into_iter().map(EntityRef::body));
                }
            }
            out.dedup();
            out
        }
        "COMPOSITE_CONTAINING" => {
            let wanted_closed = match field(query, "compositePartType").as_str() {
                Some("CLOSED") => Some(true),
                Some("OPEN") => Some(false),
                _ => None,
            };
            let mut out: Vec<EntityRef> = Vec::new();
            for r in sub(model, query, "bodies")? {
                for c in model.composites_containing(r.body) {
                    let closed = model.bodies[c].composite.as_ref().is_some_and(|(_, closed)| *closed);
                    if wanted_closed.is_none_or(|w| w == closed) && !out.contains(&EntityRef::body(c)) {
                        out.push(EntityRef::body(c));
                    }
                }
            }
            out
        }
        // Nothing is locked in a context the engine builds itself.
        "MODIFIABLE_ENTITY_FILTER" => sub(model, query, "subquery")?,
        "OWNED_BY_PART" => {
            let kind = entity_type(field(query, "entityType"))?;
            // Only bodies own anything: faces, edges and vertices in `part` are ignored.
            let bodies: BTreeSet<usize> =
                sub(model, query, "part")?.into_iter().filter(|r| r.kind == EntityType::Body).map(|r| r.body).collect();
            let owned: Vec<EntityRef> = bodies.into_iter().flat_map(|b| model.owned_by(b, kind)).collect();
            if field(query, "subquery").is_undefined() {
                owned
            } else {
                let within: BTreeSet<EntityRef> = owned.into_iter().collect();
                sub(model, query, "subquery")?.into_iter().filter(|r| within.contains(r)).collect()
            }
        }
        "OWNER_PART" => sub(model, query, "query")?.into_iter().map(|r| EntityRef::body(r.body)).collect(),
        "CAP_ENTITY" => {
            let feature = values::feature_id(field(query, "featureId"), "qCapEntity")?;
            let cap = match field(query, "capType").as_str() {
                Some("START") => Some(crate::model::Cap::Start),
                Some("END") => Some(crate::model::Cap::End),
                _ => None,
            };
            let kind = entity_type(field(query, "entityType"))?;
            model
                .everything(kind)
                .into_iter()
                .filter(|r| r.kind != EntityType::Body)
                .filter(|r| {
                    let info = model.info(*r);
                    info.is_created_by(&feature) && info.cap.is_some() && (cap.is_none() || info.cap == cap)
                })
                .collect()
        }
        "GEOMETRY" => {
            let wanted = field(query, "geometryType");
            let wanted: Vec<&str> = match wanted.as_array() {
                Some(items) => items.iter().filter_map(Value::as_str).collect(),
                None => wanted.as_str().into_iter().collect(),
            };
            sub(model, query, "subquery")?
                .into_iter()
                .filter(|r| geometry_type(model, *r).is_some_and(|t| wanted.contains(&t)))
                .collect()
        }
        other => return Err(error(format!("evaluateQuery: QueryType.{other} isn't supported yet"))),
    })
}

/// A tracking query (`startTracking`): the entities made, since tracking started, from the entities it tracks (and
/// from the secondary ones too, if given). The tracked entities themselves aren't included.
fn tracking(model: &Model, query: &Value) -> R<Vec<EntityRef>> {
    let tids = |key: &str| -> R<Option<Vec<crate::model::Tid>>> {
        let list = field(query, key);
        if list.is_undefined() {
            return Ok(None);
        }
        let mut out = Vec::new();
        for item in list.as_array().map(|items| items.iter()).into_iter().flatten() {
            let tid = field(item, "transientId").as_str().and_then(parse_tid);
            out.extend(tid);
        }
        Ok(Some(out))
    };
    let first = tids("subquery1")?.unwrap_or_default();
    let second = tids("subquery2")?;
    let identity_only = field(query, "identityPreservingOnly").as_bool() == Some(true);
    let mut out = Vec::new();
    for (b, body) in model.bodies.iter().enumerate() {
        // Bodies made from a tracked body: the pieces a split or boolean left of it.
        if !first.contains(&body.tid) {
            let ancestors = if identity_only { &body.ancestry.identity } else { &body.ancestry.all };
            let from = |tids: &[crate::model::Tid]| tids.iter().any(|t| ancestors.contains(t));
            if from(&first) && second.as_deref().is_none_or(from) {
                out.push(EntityRef::body(b));
            }
        }
        for kind in [EntityType::Face, EntityType::Edge, EntityType::Vertex] {
            for (index, entity) in body.entities(kind).iter().enumerate() {
                if first.contains(&entity.tid) {
                    continue;
                }
                let ancestors = if identity_only { &entity.ancestry.identity } else { &entity.ancestry.all };
                let from = |tids: &[crate::model::Tid]| tids.iter().any(|t| ancestors.contains(t));
                if from(&first) && second.as_deref().is_none_or(from) {
                    out.push(EntityRef { body: b, kind, index });
                }
            }
        }
    }
    Ok(out)
}

fn keep_if(found: Vec<EntityRef>, keep: bool) -> Vec<EntityRef> {
    if keep { found } else { Vec::new() }
}

/// std's `GeometryType` of a face or edge.
pub fn geometry_type(model: &Model, r: EntityRef) -> Option<&'static str> {
    let shape = model.shape(r);
    match r.kind {
        EntityType::Face => Some(match shape.surface_type().ok()? {
            occt::SurfaceType::Plane => "PLANE",
            occt::SurfaceType::Cylinder => "CYLINDER",
            occt::SurfaceType::Cone => "CONE",
            occt::SurfaceType::Sphere => "SPHERE",
            occt::SurfaceType::Torus => "TORUS",
            occt::SurfaceType::Revolution => "REVOLVED",
            occt::SurfaceType::Extrusion => "EXTRUDED",
            _ => "OTHER_SURFACE",
        }),
        EntityType::Edge => Some(match shape.curve_type().ok()? {
            occt::CurveType::Line => "LINE",
            occt::CurveType::Circle => {
                if shape.sub_shapes(ShapeKind::Vertex).len() <= 1 {
                    "CIRCLE"
                } else {
                    "ARC"
                }
            }
            occt::CurveType::Ellipse => "ELLIPSE",
            _ => "OTHER_CURVE",
        }),
        _ => None,
    }
}

/// The region faces of the sketches `feature` created. With `filter_inner`, regions that fill a hole in another
/// region are left out: their whole boundary is shared with that region.
/// The edges and vertices of a sketch's regions that lie on some of its curves and points.
fn imprints_of(model: &Model, feature: &[std::rc::Rc<str>], from: &[EntityRef]) -> Vec<EntityRef> {
    let on = |shape: &occt::Shape, target: &occt::Shape| -> bool {
        let probe = |p: occt::Vec3| occt::vertex(p).and_then(|v| v.distance(target)).is_ok_and(|d| d.distance < 1e-9);
        match shape.kind() {
            Some(ShapeKind::Vertex) => shape.vertex_point().is_ok_and(probe),
            _ => [0.0, 0.5, 1.0].iter().all(|&t| shape.edge_tangent(t, false).is_ok_and(|a| probe(a.origin))),
        }
    };
    let mut out = Vec::new();
    for (b, body) in model.bodies.iter().enumerate() {
        if body.sketch_plane.is_none()
            || body.body_type != crate::model::BodyType::Sheet
            || !body.info.is_created_by(feature)
        {
            continue;
        }
        for kind in [EntityType::Edge, EntityType::Vertex] {
            for (index, entity) in body.entities(kind).iter().enumerate() {
                let hit = from.iter().any(|r| {
                    let target = model.shape(*r);
                    match (kind, r.kind) {
                        (EntityType::Edge, EntityType::Edge) | (EntityType::Vertex, _) => on(&entity.shape, target),
                        _ => false,
                    }
                });
                if hit {
                    out.push(EntityRef { body: b, kind, index });
                }
            }
        }
    }
    out
}

fn sketch_regions(model: &Model, feature: &[std::rc::Rc<str>], filter_inner: bool) -> Vec<EntityRef> {
    let mut regions = Vec::new();
    for (b, body) in model.bodies.iter().enumerate() {
        if body.sketch_plane.is_none() || !body.info.is_created_by(feature) {
            continue;
        }
        regions.extend((0..body.faces.len()).map(|index| EntityRef { body: b, kind: EntityType::Face, index }));
    }
    if !filter_inner {
        return regions;
    }
    let edges: Vec<Vec<occt::Shape>> = regions.iter().map(|r| model.shape(*r).sub_shapes(ShapeKind::Edge)).collect();
    regions
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            !edges
                .iter()
                .enumerate()
                .any(|(j, other)| *i != j && edges[*i].iter().all(|e| other.iter().any(|o| o.is_same(e))))
        })
        .map(|(_, r)| *r)
        .collect()
}
