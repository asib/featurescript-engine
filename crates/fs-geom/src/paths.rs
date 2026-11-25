//! Builtins that organise geometry: `@constructPaths` (edges into ordered paths), `@clusterPoints`, and
//! `@lastModifyingOperationId`.

use crate::model::{EntityRef, EntityType, Model};
use crate::query::tid_string;
use crate::values::{self, Tags, field, map};
use crate::{GeomBuiltins, model_ref, query, tags};
use fs_interp::Value;
use fs_interp::builtins::arg;
use fs_interp::eval::{R, error};
use occt::Shape;

/// Edges chained end to end: each path as (edges with whether each runs backwards, closed).
fn chains(model: &Model, edges: &[EntityRef]) -> Vec<(Vec<(EntityRef, bool)>, bool)> {
    // Each edge's ends, its start first as Onshape's edge runs.
    let ends: Vec<Vec<Shape>> = edges.iter().map(|r| crate::topology::end_vertices(model, *r)).collect();
    let mut used = vec![false; edges.len()];
    let mut out = Vec::new();
    let meets = |a: &Shape, b: &Shape| a.is_same(b) || a.distance(b).is_ok_and(|d| d.distance < 1e-8);
    for start in 0..edges.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut chain = vec![(start, false)];
        // Grow forward from the end, then backward from the start.
        for forward in [true, false] {
            loop {
                let (last, flipped) = if forward { *chain.last().expect("chain") } else { chain[0] };
                let e = &ends[last];
                if e.len() < 2 {
                    break;
                }
                let tip = if forward != flipped { &e[1] } else { &e[0] };
                let next = (0..edges.len())
                    .find(|&i| !used[i] && ends[i].len() == 2 && (meets(&ends[i][0], tip) || meets(&ends[i][1], tip)));
                let Some(i) = next else { break };
                used[i] = true;
                // Forward: the next edge should start at the tip; backward: end at it.
                let starts_at_tip = meets(&ends[i][0], tip);
                let flip = if forward { !starts_at_tip } else { starts_at_tip };
                if forward {
                    chain.push((i, flip));
                } else {
                    chain.insert(0, (i, flip));
                }
            }
        }
        let first = &ends[chain[0].0];
        let last = &ends[chain.last().expect("chain").0];
        let start_point = if chain[0].1 { first.get(1) } else { first.first() };
        let end_point = if chain.last().expect("chain").1 { last.first() } else { last.get(1) };
        let closed = match (start_point, end_point) {
            (Some(a), Some(b)) => meets(a, b),
            _ => {
                ends[chain[0].0].len() < 2
                    || (ends[chain[0].0].len() == 2 && ends[chain[0].0][0].is_same(&ends[chain[0].0][1]))
            }
        };
        out.push((chain.into_iter().map(|(i, f)| (edges[i], f)).collect(), closed));
    }
    out
}

fn construct_paths(tags: &Tags, model: &Model, definition: &Value) -> R<Value> {
    let edges: Vec<EntityRef> = query::evaluate(model, field(definition, "edges"))?
        .into_iter()
        .filter(|r| r.kind == EntityType::Edge)
        .collect();
    let mut paths = Vec::new();
    for (chain, closed) in chains(model, &edges) {
        let queries = chain.iter().map(|(r, _)| tags.transient_query(&tid_string(model.tid(*r)))).collect();
        let flipped = chain.iter().map(|(_, f)| Value::bool(*f)).collect();
        let path = map(vec![
            ("edges", Value::array(queries)),
            ("flipped", Value::array(flipped)),
            ("closed", Value::bool(closed)),
        ]);
        paths.push(tags.tagged(path, "Path")?);
    }
    Ok(Value::array(paths))
}

pub fn register(m: &mut GeomBuiltins) {
    m.insert("constructPaths", |interp, args| {
        let tags = tags(interp)?;
        let definition = arg(&args, 1, "constructPaths")?.clone();
        model_ref(&args, "constructPaths", |model| construct_paths(&tags, model, &definition))
    });
    // Groups of points within `tolerance` of each other (transitively), as arrays of indices.
    m.insert("clusterPoints", |_, args| {
        let points = arg(&args, 0, "clusterPoints")?
            .as_array()
            .ok_or_else(|| error("@clusterPoints: points must be an array"))?
            .iter()
            .map(|p| values::vec3(p, "clusterPoints point"))
            .collect::<R<Vec<_>>>()?;
        let tolerance =
            arg(&args, 1, "clusterPoints")?.as_number().ok_or_else(|| error("@clusterPoints: tolerance"))?;
        let mut cluster = vec![usize::MAX; points.len()];
        let mut clusters: Vec<Vec<usize>> = Vec::new();
        for i in 0..points.len() {
            if cluster[i] != usize::MAX {
                continue;
            }
            let c = clusters.len();
            clusters.push(vec![i]);
            cluster[i] = c;
            let mut k = 0;
            while k < clusters[c].len() {
                let p = points[clusters[c][k]];
                for j in 0..points.len() {
                    if cluster[j] == usize::MAX && values::norm(values::sub(points[j], p)) <= tolerance {
                        cluster[j] = c;
                        clusters[c].push(j);
                    }
                }
                k += 1;
            }
        }
        Ok(Value::array(
            clusters
                .into_iter()
                .map(|c| Value::array(c.into_iter().map(|i| Value::number(i as f64)).collect()))
                .collect(),
        ))
    });
    m.insert("lastOperationId", |interp, args| {
        let tags = tags(interp)?;
        model_ref(&args, "lastOperationId", |model| {
            Ok(values::id_value(&tags, model.last_operation.as_deref().unwrap_or(&[])))
        })
    });
    // The operation that last changed an entity: the one that made it, as the engine tracks it.
    m.insert("lastModifyingOperationId", |interp, args| {
        let tags = tags(interp)?;
        let definition = arg(&args, 1, "lastModifyingOperationId")?.clone();
        model_ref(&args, "lastModifyingOperationId", |model| {
            let found = query::evaluate(model, field(&definition, "entity"))?;
            let r = found.first().ok_or_else(|| crate::regen_error("CANNOT_RESOLVE_ENTITIES"))?;
            Ok(values::id_value(&tags, &model.info(*r).created_by))
        })
    });
}
