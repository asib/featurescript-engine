//! Tessellated lofts (`sheetMetalLoft`): a sheet of flat facets between two profiles, matched point to point,
//! arcs split into chords within the chordal tolerance. Only profiles of lines and arcs are supported yet.

use crate::model::{BodyType, EntityRef, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, Tags, field};
use crate::{occt_error, query, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Shape, Vec3};

/// An edge of a profile's chain, run from `from` to `to`.
struct Link {
    edge: EntityRef,
    from: Vec3,
    to: Vec3,
    /// Whether the chain runs against the edge's own direction.
    reversed: bool,
}

/// A profile: its edges in order along it, and whether it closes on itself.
struct Profile {
    links: Vec<Link>,
    closed: bool,
}

impl Profile {
    /// The `i`th vertex along the profile (0 to `links.len()`; a closed profile's last is its first).
    fn vertex(&self, i: usize) -> Vec3 {
        if i < self.links.len() { self.links[i].from } else { self.links[self.links.len() - 1].to }
    }

    /// The `i`th vertex as Onshape gives it in a match: on the edge ending there, at its own parameter (the first on
    /// the first edge, where it starts; batch 077).
    fn vertex_on_edge(&self, i: usize) -> (Vec3, EntityRef, f64) {
        let (link, at_start) = if i == 0 { (&self.links[0], true) } else { (&self.links[i - 1], false) };
        let t = if at_start != link.reversed { 0.0 } else { 1.0 };
        (self.vertex(i), link.edge, t)
    }
}

/// A profile's edges in order along it, and whether it closes on itself. Vertices meet where they coincide: a
/// sketch's lines are wire bodies of their own (an L profile, batch 044).
fn profile_chain(model: &Model, profile: &Value) -> R<Profile> {
    let edges: Vec<EntityRef> =
        query::evaluate(model, profile)?.into_iter().filter(|r| r.kind == EntityType::Edge).collect();
    if edges.is_empty() {
        return Err(regen_error("TL_SELECT_PROFILES"));
    }
    // Each edge's two ends, in its own direction.
    let mut points: Vec<[Vec3; 2]> = Vec::new();
    for r in &edges {
        let shape = model.shape(*r);
        if !matches!(shape.curve_type().ok(), Some(occt::CurveType::Line | occt::CurveType::Circle)) {
            return Err(error("@opTessellatedLoft: only profiles of lines and arcs are supported yet"));
        }
        let end = |t: f64| shape.edge_tangent(t, false).map(|l| l.origin).map_err(occt_error);
        points.push([end(0.0)?, end(1.0)?]);
    }
    let same = |p: Vec3, q: Vec3| values::norm(values::sub(p, q)) < 1e-9;
    let count = |p: Vec3| points.iter().flatten().filter(|&&q| same(p, q)).count();
    let start = (0..points.len()).flat_map(|i| [(i, 0), (i, 1)]).find(|&(i, k)| count(points[i][k]) == 1);
    let closed = start.is_none();
    let (i0, k0) = start.unwrap_or((0, 0));
    let mut current = points[i0][k0];
    let first = current;
    let mut used = vec![false; points.len()];
    let mut links = Vec::new();
    while let Some(i) = (0..points.len()).find(|&i| !used[i] && points[i].iter().any(|&q| same(q, current))) {
        used[i] = true;
        let k = if same(points[i][0], current) { 1 } else { 0 };
        let next = points[i][k];
        links.push(Link { edge: edges[i], from: current, to: next, reversed: k == 0 });
        current = next;
        if closed && same(current, first) {
            break;
        }
    }
    if used.iter().any(|u| !u) {
        return Err(error("@opTessellatedLoft: a profile must be one chain of edges"));
    }
    Ok(Profile { links, closed })
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

/// The points along an edge of a profile, in `n` equal chords (by length; an arc's by angle), both ends included.
fn link_points(model: &Model, link: &Link, n: usize) -> R<Vec<Vec3>> {
    let shape = model.shape(link.edge);
    let mut out = vec![link.from];
    for k in 1..n {
        let along = k as f64 / n as f64;
        let t = if link.reversed { 1.0 - along } else { along };
        out.push(shape.edge_tangent(t, true).map_err(occt_error)?.origin);
    }
    out.push(link.to);
    Ok(out)
}

/// The two profiles (the second turned to run the same way as the first) and their vertices matched in order: index
/// pairs from (0, 0) to the two last, each step on by one along either or both. The matching is the one with the
/// least total distance between matched vertices, so an L to a line fans its corner to the line's nearer end (batch
/// 077 c001). Where a step along both runs between two lines whose ends don't lie on a plane, the shorter diagonal is
/// a match of its own (c003).
fn matched(model: &Model, definition: &Value) -> R<(Profile, Profile, Vec<(usize, usize)>)> {
    let profiles = field(definition, "profileSubqueries")
        .as_array()
        .ok_or_else(|| error("@opTessellatedLoft: profileSubqueries must be an array"))?
        .clone();
    if profiles.len() != 2 {
        return Err(error("@opTessellatedLoft: profileSubqueries must hold two profiles"));
    }
    let first = profile_chain(model, &profiles[0])?;
    let mut second = profile_chain(model, &profiles[1])?;
    if first.closed != second.closed {
        return Err(error("@opTessellatedLoft: an open profile can't be lofted to a closed one"));
    }
    let run = |p: &Profile| values::sub(p.vertex(p.links.len()), p.vertex(0));
    if !first.closed && values::dot(run(&first), run(&second)) < 0.0 {
        second.links.reverse();
        for link in second.links.iter_mut() {
            std::mem::swap(&mut link.from, &mut link.to);
            link.reversed = !link.reversed;
        }
    }
    let (m, n) = (first.links.len(), second.links.len());
    let distance = |i: usize, j: usize| values::norm(values::sub(first.vertex(i), second.vertex(j)));
    // The least total distance to each pair, and the step there.
    let mut cost = vec![vec![f64::INFINITY; n + 1]; m + 1];
    let mut back = vec![vec![(0, 0); n + 1]; m + 1];
    cost[0][0] = distance(0, 0);
    for i in 0..=m {
        for j in 0..=n {
            if i == 0 && j == 0 {
                continue;
            }
            for (di, dj) in [(1, 1), (1, 0), (0, 1)] {
                if i >= di && j >= dj && cost[i - di][j - dj] + distance(i, j) < cost[i][j] {
                    cost[i][j] = cost[i - di][j - dj] + distance(i, j);
                    back[i][j] = (di, dj);
                }
            }
        }
    }
    let mut path = vec![(m, n)];
    while path[path.len() - 1] != (0, 0) {
        let (i, j) = path[path.len() - 1];
        let (di, dj) = back[i][j];
        path.push((i - di, j - dj));
    }
    path.reverse();
    let is_line = |link: &Link| model.shape(link.edge).curve_type().ok() == Some(occt::CurveType::Line);
    let mut pairs = vec![path[0]];
    for w in path.windows(2) {
        let ((i, j), next) = (w[0], w[1]);
        if next == (i + 1, j + 1) && is_line(&first.links[i]) && is_line(&second.links[j]) {
            let (a, b, c, d) = (first.vertex(i), first.vertex(i + 1), second.vertex(j + 1), second.vertex(j));
            let normal = values::normalize(values::cross(values::sub(b, a), values::sub(d, a)));
            if normal.is_some_and(|n| values::dot(values::sub(c, a), n).abs() > 1e-9) {
                pairs.push(if values::norm(values::sub(d, b)) < values::norm(values::sub(c, a)) {
                    (i + 1, j)
                } else {
                    (i, j + 1)
                });
            }
        }
        pairs.push(next);
    }
    Ok((first, second, pairs))
}

/// `@evTessellatedLoftMatches`: the profiles' vertices matched in order, each as a point on an edge.
pub fn matches(tags: &Tags, model: &Model, definition: &Value) -> R<Value> {
    let (first, second, mut pairs) = matched(model, definition)?;
    if first.closed {
        pairs.pop();
    }
    let item = |(p, e, t): (Vec3, EntityRef, f64)| {
        values::map(vec![
            ("position", tags.length_vector(p)),
            ("edge", tags.transient_query(&query::tid_string(model.tid(e)))),
            ("parameter", Value::number(t)),
        ])
    };
    let list = pairs
        .iter()
        .map(|&(i, j)| {
            let pair = vec![item(first.vertex_on_edge(i)), item(second.vertex_on_edge(j))];
            values::map(vec![("match", Value::array(pair)), ("hideMatchFromUI", Value::bool(false))])
        })
        .collect();
    Ok(values::map(vec![("matches", Value::array(list)), ("isClosed", Value::bool(first.closed))]))
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

/// `@opTessellatedLoft`: a sheet of facets between the matched vertices. A step along both profiles is a band
/// between their two edges, each split into as many chords as the one needing more needs (batch 044 c004: arcs of
/// radius 20 and 10 both in 8), a flat quadrilateral between each two pairs of points or two triangles where the
/// four don't lie on a plane; a step along one is a fan of triangles from its edge's chords to the other's vertex.
/// The sheet faces along the first profile crossed with the way to the second (batch 017).
pub fn loft(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let (first, second, pairs) = matched(model, definition)?;
    let tolerance =
        values::length(field(definition, "chordalTolerance"), "opTessellatedLoft chordalTolerance").unwrap_or(1e-4);
    let normal = |p: Vec3, q: Vec3, r: Vec3| {
        values::normalize(values::cross(values::sub(q, p), values::sub(r, p))).ok_or_else(|| regen_error("TL_FAILED"))
    };
    let triangle = |p: Vec3, q: Vec3, r: Vec3| facet(&[p, q, r], normal(p, q, r)?);
    let mut faces = Vec::new();
    for w in pairs.windows(2) {
        let ((i, j), (i2, j2)) = (w[0], w[1]);
        match (i2 - i, j2 - j) {
            (1, 1) => {
                let (p, q) = (&first.links[i], &second.links[j]);
                let n = chords(model, p, tolerance)?.max(chords(model, q, tolerance)?);
                let (ps, qs) = (link_points(model, p, n)?, link_points(model, q, n)?);
                for k in 0..n {
                    let (a, b, c, d) = (ps[k], ps[k + 1], qs[k + 1], qs[k]);
                    let quad_normal = normal(a, b, d)?;
                    if values::dot(values::sub(c, a), quad_normal).abs() < 1e-9 {
                        faces.push(facet(&[a, b, c, d], quad_normal)?);
                    } else if values::norm(values::sub(d, b)) < values::norm(values::sub(c, a)) {
                        // Split along the shorter diagonal (batch 044: a line lofted to a skew line).
                        faces.push(triangle(a, b, d)?);
                        faces.push(triangle(b, c, d)?);
                    } else {
                        faces.push(triangle(a, b, c)?);
                        faces.push(triangle(a, c, d)?);
                    }
                }
            }
            (1, 0) => {
                let link = &first.links[i];
                let ps = link_points(model, link, chords(model, link, tolerance)?)?;
                let apex = second.vertex(j);
                for k in 0..ps.len() - 1 {
                    faces.push(triangle(ps[k], ps[k + 1], apex)?);
                }
            }
            _ => {
                let link = &second.links[j];
                let qs = link_points(model, link, chords(model, link, tolerance)?)?;
                let apex = first.vertex(i);
                for k in 0..qs.len() - 1 {
                    faces.push(triangle(apex, qs[k + 1], qs[k])?);
                }
            }
        }
    }
    let sheet = if faces.len() == 1 { faces.remove(0) } else { occt::sew_faces(&faces, 1e-7).map_err(occt_error)? };
    let info = Info::new(id);
    model.add_body(NewBody::new(sheet, BodyType::Sheet, info.clone()), &Origins::default(), &info, &|_| None);
    Ok(Value::UNDEFINED)
}
