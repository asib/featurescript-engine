//! Tessellated lofts (`sheetMetalLoft`): a sheet of flat facets between two profiles, matched point to point.
//! Only profiles of straight edges with as many vertices as each other are supported yet.

use crate::model::{BodyType, EntityRef, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, Tags, field};
use crate::{occt_error, query, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Shape, ShapeKind, Vec3};

/// Points along a profile, each with its vertex.
type Points = Vec<(Vec3, EntityRef)>;

/// The points along a profile of straight edges, in order, each with its vertex; and whether it closes on itself.
fn profile_points(model: &Model, profile: &Value) -> R<(Points, bool)> {
    let edges: Vec<EntityRef> =
        query::evaluate(model, profile)?.into_iter().filter(|r| r.kind == EntityType::Edge).collect();
    if edges.is_empty() {
        return Err(regen_error("TL_SELECT_PROFILES"));
    }
    // Each edge as its two end vertices.
    let mut segments: Vec<[EntityRef; 2]> = Vec::new();
    for r in &edges {
        let shape = model.shape(*r);
        if shape.curve_type().ok() != Some(occt::CurveType::Line) {
            return Err(error("@opTessellatedLoft: only profiles of straight edges are supported yet"));
        }
        let body = &model.bodies[r.body];
        let ends: Vec<EntityRef> = shape
            .sub_shapes(ShapeKind::Vertex)
            .iter()
            .filter_map(|v| body.vertices.iter().position(|x| x.shape.is_same(v)))
            .map(|index| EntityRef { body: r.body, kind: EntityType::Vertex, index })
            .collect();
        let [a, b] = ends[..] else { return Err(error("@opTessellatedLoft: an edge of a profile has no two ends")) };
        segments.push([a, b]);
    }
    let point = |r: EntityRef| model.shape(r).vertex_point().map_err(occt_error);
    let count = |v: EntityRef| segments.iter().flatten().filter(|&&x| x == v).count();
    let start = segments.iter().flatten().copied().find(|&v| count(v) == 1);
    let closed = start.is_none();
    let mut current = start.unwrap_or(segments[0][0]);
    let mut used = vec![false; segments.len()];
    let mut out = vec![(point(current)?, current)];
    while let Some(i) = (0..segments.len()).find(|&i| !used[i] && segments[i].contains(&current)) {
        used[i] = true;
        current = if segments[i][0] == current { segments[i][1] } else { segments[i][0] };
        if closed && current == out[0].1 {
            break;
        }
        out.push((point(current)?, current));
    }
    if used.iter().any(|u| !u) {
        return Err(error("@opTessellatedLoft: a profile must be one chain of edges"));
    }
    Ok((out, closed))
}

/// The two profiles' points, the second turned to run the same way as the first.
fn matched(model: &Model, definition: &Value) -> R<(Points, Points, bool)> {
    let profiles = field(definition, "profileSubqueries")
        .as_array()
        .ok_or_else(|| error("@opTessellatedLoft: profileSubqueries must be an array"))?
        .clone();
    if profiles.len() != 2 {
        return Err(error("@opTessellatedLoft: profileSubqueries must hold two profiles"));
    }
    let (first, closed) = profile_points(model, &profiles[0])?;
    let (mut second, closed2) = profile_points(model, &profiles[1])?;
    if first.len() != second.len() || closed != closed2 {
        return Err(error("@opTessellatedLoft: only profiles with as many vertices as each other are supported yet"));
    }
    let run = |p: &[(Vec3, EntityRef)]| values::sub(p[p.len() - 1].0, p[0].0);
    if !closed && values::dot(run(&first), run(&second)) < 0.0 {
        second.reverse();
    }
    Ok((first, second, closed))
}

/// `@evTessellatedLoftMatches`: the profiles' points matched in order.
pub fn matches(tags: &Tags, model: &Model, definition: &Value) -> R<Value> {
    let (first, second, closed) = matched(model, definition)?;
    let item = |(p, v): (Vec3, EntityRef)| {
        values::map(vec![
            ("position", tags.length_vector(p)),
            ("vertex", tags.transient_query(&query::tid_string(model.tid(v)))),
        ])
    };
    let list = first
        .iter()
        .zip(&second)
        .map(|(&a, &b)| {
            values::map(vec![("match", Value::array(vec![item(a), item(b)])), ("hideMatchFromUI", Value::bool(false))])
        })
        .collect();
    Ok(values::map(vec![("matches", Value::array(list)), ("isClosed", Value::bool(closed))]))
}

/// A flat polygon face, its normal along `normal`.
fn facet(points: &[Vec3], normal: Vec3) -> R<Shape> {
    let edges = (0..points.len())
        .map(|i| occt::line_edge(points[i], points[(i + 1) % points.len()]).map_err(occt_error))
        .collect::<R<Vec<_>>>()?;
    let x = values::normalize(values::sub(points[1], points[0])).ok_or_else(|| regen_error("TL_FAILED"))?;
    let frame = occt::Frame { origin: points[0], normal, x };
    occt::sketch_regions(&edges, frame).map_err(occt_error)?.into_iter().next().ok_or_else(|| regen_error("TL_FAILED"))
}

/// `@opTessellatedLoft`: a sheet of facets between the matched points: a flat quadrilateral between each two
/// pairs, or two triangles where the four don't lie on a plane. The sheet faces along the first profile crossed
/// with the way to the second (batch 017).
pub fn loft(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let (first, second, closed) = matched(model, definition)?;
    let n = first.len();
    let spans = if closed { n } else { n - 1 };
    let mut faces = Vec::new();
    for i in 0..spans {
        let j = (i + 1) % n;
        let (a, b, c, d) = (first[i].0, first[j].0, second[j].0, second[i].0);
        let normal = |p: Vec3, q: Vec3, r: Vec3| values::normalize(values::cross(values::sub(q, p), values::sub(r, p)));
        let quad_normal = normal(a, b, d).ok_or_else(|| regen_error("TL_FAILED"))?;
        let off_plane = values::dot(values::sub(c, a), quad_normal).abs();
        if off_plane < 1e-9 {
            faces.push(facet(&[a, b, c, d], quad_normal)?);
        } else {
            faces.push(facet(&[a, b, c], normal(a, b, c).ok_or_else(|| regen_error("TL_FAILED"))?)?);
            faces.push(facet(&[a, c, d], normal(a, c, d).ok_or_else(|| regen_error("TL_FAILED"))?)?);
        }
    }
    let sheet = if faces.len() == 1 { faces.remove(0) } else { occt::sew_faces(&faces, 1e-7).map_err(occt_error)? };
    let info = Info::new(id);
    model.add_body(NewBody::new(sheet, BodyType::Sheet, info.clone()), &Origins::default(), &info, &|_| None);
    Ok(Value::UNDEFINED)
}
