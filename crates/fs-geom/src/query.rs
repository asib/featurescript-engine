//! Evaluating std's queries: maps with a `queryType` that `@evaluateQuery` resolves against the model.

use crate::model::{EntityRef, EntityType, Model, Tid};
use crate::values::{self, field};
use crate::{GeomBuiltins, model_mut, tags};
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
        "EVERYTHING" => model.everything(entity_type(field(query, "entityType"))?),
        "TRANSIENT" => {
            let id = field(query, "transientId").as_str().and_then(parse_tid);
            id.and_then(|tid| model.find_tid(tid)).into_iter().collect()
        }
        "CREATED_BY" => {
            let feature = values::feature_id(field(query, "featureId"), "qCreatedBy")?;
            let kind = entity_type(field(query, "entityType"))?;
            model.everything(kind).into_iter().filter(|r| model.info(*r).is_created_by(&feature)).collect()
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
        "SM_FLAT_FILTER" => keep_if(sub(model, query, "subquery")?, !yes_no(query, "flatFilter")?),
        "SM_FORM_FILTER" => keep_if(sub(model, query, "subquery")?, !yes_no(query, "formFilter")?),
        "ACTIVE_SM_FILTER" => keep_if(sub(model, query, "subquery")?, !yes_no(query, "activeSheetMetal")?),
        // Meshes aren't modelled, so nothing is mesh geometry.
        "MESH_GEOMETRY_FILTER" => keep_if(sub(model, query, "subquery")?, !yes_no(query, "meshGeometryFilter")?),
        // Nothing is locked in a context the engine builds itself.
        "MODIFIABLE_ENTITY_FILTER" => sub(model, query, "subquery")?,
        "OWNED_BY_PART" => {
            let kind = entity_type(field(query, "entityType"))?;
            let bodies: BTreeSet<usize> = sub(model, query, "part")?.into_iter().map(|r| r.body).collect();
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
