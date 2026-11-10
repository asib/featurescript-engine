//! Tessellated lofts (`sheetMetalLoft`): a sheet of flat facets between two profiles, matched point to point,
//! arcs split into chords within the chordal tolerance. Only profiles of lines and arcs with as many edges as each
//! other are supported yet.

use crate::model::{BodyType, EntityRef, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, Tags, field};
use crate::{occt_error, query, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Shape, Vec3};

/// What a point along a profile is: one of its vertices, or a point a fraction along one of its edges (an arc's
/// tessellation).
#[derive(Clone, Copy)]
enum At {
    Vertex(EntityRef),
    Edge(EntityRef, f64),
}

/// Points along a profile, each with what it is.
type Points = Vec<(Vec3, At)>;

/// An edge of a profile's chain, run from `from` to `to`.
struct Link {
    edge: EntityRef,
    from: (Vec3, EntityRef),
    to: (Vec3, EntityRef),
    /// Whether the chain runs against the edge's own direction.
    reversed: bool,
}

/// A profile's edges in order along it, and whether it closes on itself. Vertices meet where they coincide: a
/// sketch's lines are wire bodies of their own (an L profile, batch 044).
fn profile_chain(model: &Model, profile: &Value) -> R<(Vec<Link>, bool)> {
    let edges: Vec<EntityRef> =
        query::evaluate(model, profile)?.into_iter().filter(|r| r.kind == EntityType::Edge).collect();
    if edges.is_empty() {
        return Err(regen_error("TL_SELECT_PROFILES"));
    }
    // Each edge as its two end vertices, in its own direction.
    let mut segments: Vec<[EntityRef; 2]> = Vec::new();
    for r in &edges {
        let shape = model.shape(*r);
        if !matches!(shape.curve_type().ok(), Some(occt::CurveType::Line | occt::CurveType::Circle)) {
            return Err(error("@opTessellatedLoft: only profiles of lines and arcs are supported yet"));
        }
        let body = &model.bodies[r.body];
        let vertex = |t: f64| -> Option<EntityRef> {
            let p = shape.edge_tangent(t, false).ok()?.origin;
            body.vertices
                .iter()
                .position(|x| x.shape.vertex_point().is_ok_and(|q| values::norm(values::sub(p, q)) < 1e-9))
                .map(|index| EntityRef { body: r.body, kind: EntityType::Vertex, index })
        };
        let (Some(a), Some(b)) = (vertex(0.0), vertex(1.0)) else {
            return Err(error("@opTessellatedLoft: an edge of a profile has no two ends"));
        };
        segments.push([a, b]);
    }
    let point = |r: EntityRef| model.shape(r).vertex_point().map_err(occt_error);
    let points: Vec<[Vec3; 2]> = segments.iter().map(|[a, b]| Ok([point(*a)?, point(*b)?])).collect::<R<_>>()?;
    let same = |p: Vec3, q: Vec3| values::norm(values::sub(p, q)) < 1e-9;
    let count = |p: Vec3| points.iter().flatten().filter(|&&q| same(p, q)).count();
    let start = (0..segments.len()).flat_map(|i| [(i, 0), (i, 1)]).find(|&(i, k)| count(points[i][k]) == 1);
    let closed = start.is_none();
    let (i0, k0) = start.unwrap_or((0, 0));
    let mut current = (points[i0][k0], segments[i0][k0]);
    let first = current.0;
    let mut used = vec![false; segments.len()];
    let mut links = Vec::new();
    while let Some(i) = (0..segments.len()).find(|&i| !used[i] && points[i].iter().any(|&q| same(q, current.0))) {
        used[i] = true;
        let k = if same(points[i][0], current.0) { 1 } else { 0 };
        let next = (points[i][k], segments[i][k]);
        links.push(Link { edge: edges[i], from: current, to: next, reversed: k == 0 });
        current = next;
        if closed && same(current.0, first) {
            break;
        }
    }
    if used.iter().any(|u| !u) {
        return Err(error("@opTessellatedLoft: a profile must be one chain of edges"));
    }
    Ok((links, closed))
}

/// How many chords an edge needs for none to stray more than `tolerance` from it: one for a line; for an arc the
/// fewest equal ones whose sagitta is within it.
fn chords(model: &Model, link: &Link, tolerance: f64) -> R<usize> {
    let shape = model.shape(link.edge);
    if shape.curve_type().ok() != Some(occt::CurveType::Circle) {
        return Ok(1);
    }
    let radius = shape.curve_definition().map_err(occt_error)?.radius1;
    let length = shape.length().map_err(occt_error)?;
    let angle = length / radius;
    let mut n = 1;
    while radius * (1.0 - (angle / (2.0 * n as f64)).cos()) > tolerance && n < 10_000 {
        n += 1;
    }
    Ok(n)
}

/// A profile's points, each edge in as many equal chords as given (by length; an arc's by angle).
fn chain_points(model: &Model, links: &[Link], counts: &[usize], closed: bool) -> R<Points> {
    let mut out: Points = Vec::new();
    for (link, &n) in links.iter().zip(counts) {
        out.push((link.from.0, At::Vertex(link.from.1)));
        let shape = model.shape(link.edge);
        for k in 1..n {
            let along = k as f64 / n as f64;
            let t = if link.reversed { 1.0 - along } else { along };
            let p = shape.edge_tangent(t, true).map_err(occt_error)?.origin;
            out.push((p, At::Edge(link.edge, t)));
        }
    }
    if !closed && let Some(last) = links.last() {
        out.push((last.to.0, At::Vertex(last.to.1)));
    }
    Ok(out)
}

/// The two profiles' points, the second turned to run the same way as the first: corresponding edges split into
/// as many chords as the one needing more needs (batch 044 c004: arcs of radius 20 and 10 both in 8).
fn matched(model: &Model, definition: &Value) -> R<(Points, Points, bool)> {
    let profiles = field(definition, "profileSubqueries")
        .as_array()
        .ok_or_else(|| error("@opTessellatedLoft: profileSubqueries must be an array"))?
        .clone();
    if profiles.len() != 2 {
        return Err(error("@opTessellatedLoft: profileSubqueries must hold two profiles"));
    }
    let tolerance = values::length(field(definition, "chordalTolerance"), "opTessellatedLoft chordalTolerance").unwrap_or(1e-4);
    let (first, closed) = profile_chain(model, &profiles[0])?;
    let (mut second, closed2) = profile_chain(model, &profiles[1])?;
    if first.len() != second.len() || closed != closed2 {
        return Err(error("@opTessellatedLoft: only profiles with as many edges as each other are supported yet"));
    }
    let run = |links: &[Link]| values::sub(links[links.len() - 1].to.0, links[0].from.0);
    if !closed && values::dot(run(&first), run(&second)) < 0.0 {
        second.reverse();
        for link in second.iter_mut() {
            std::mem::swap(&mut link.from, &mut link.to);
            link.reversed = !link.reversed;
        }
    }
    let counts: Vec<usize> = first
        .iter()
        .zip(&second)
        .map(|(a, b)| Ok(chords(model, a, tolerance)?.max(chords(model, b, tolerance)?)))
        .collect::<R<_>>()?;
    Ok((chain_points(model, &first, &counts, closed)?, chain_points(model, &second, &counts, closed)?, closed))
}

/// `@evTessellatedLoftMatches`: the profiles' points matched in order, each a vertex or a point along an edge.
pub fn matches(tags: &Tags, model: &Model, definition: &Value) -> R<Value> {
    let (first, second, closed) = matched(model, definition)?;
    let item = |(p, at): (Vec3, At)| match at {
        At::Vertex(v) => values::map(vec![
            ("position", tags.length_vector(p)),
            ("vertex", tags.transient_query(&query::tid_string(model.tid(v)))),
        ]),
        At::Edge(e, t) => values::map(vec![
            ("position", tags.length_vector(p)),
            ("edge", tags.transient_query(&query::tid_string(model.tid(e)))),
            ("parameter", Value::number(t)),
        ]),
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
        } else if values::norm(values::sub(d, b)) < values::norm(values::sub(c, a)) {
            // Split along the shorter diagonal (batch 044: a line lofted to a skew line).
            faces.push(facet(&[a, b, d], normal(a, b, d).ok_or_else(|| regen_error("TL_FAILED"))?)?);
            faces.push(facet(&[b, c, d], normal(b, c, d).ok_or_else(|| regen_error("TL_FAILED"))?)?);
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
