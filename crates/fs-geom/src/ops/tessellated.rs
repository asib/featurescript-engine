//! Tessellated lofts (`sheetMetalLoft`): a sheet of flat facets between two profiles, matched point to point,
//! arcs split into chords within the chordal tolerance. Only profiles of lines and arcs are supported yet.

use crate::model::{BodyType, EntityRef, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, Tags, field};
use crate::{occt_error, query, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Shape, ShapeKind, Vec3};

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

    /// The `i`th vertex as Onshape gives it in a match: on the edge ending there, at its own parameter, or where a
    /// stretch of the matching starts (the first vertex, or one a connection holds), on the edge starting there
    /// (batches 077, 078 c004).
    fn vertex_on_edge(&self, i: usize, starts: bool) -> (Vec3, EntityRef, f64) {
        let (link, at_start) = if i == 0 || (starts && i < self.links.len()) {
            (&self.links[i], true)
        } else {
            (&self.links[i - 1], false)
        };
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

/// A connection between the profiles: the vertex of each it holds together.
struct Connection {
    vertices: [usize; 2],
}

/// The connections asked for (`connections`: each its `connectionEntities` vertices and its `connectionEdges` at
/// `connectionEdgeParameters`), each naming a vertex of either profile.
fn connections(model: &Model, definition: &Value, profiles: [&Profile; 2]) -> R<Vec<Connection>> {
    let mut out = Vec::new();
    for c in field(definition, "connections").as_array().map(|a| a.to_vec()).unwrap_or_default() {
        let mut named: Vec<Vec3> = Vec::new();
        for r in query::evaluate(model, field(&c, "connectionEntities"))? {
            if r.kind == EntityType::Vertex {
                named.push(model.shape(r).vertex_point().map_err(occt_error)?);
            }
        }
        let parameters: Vec<f64> = field(&c, "connectionEdgeParameters")
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_number).collect())
            .unwrap_or_default();
        let edges: Vec<EntityRef> = match field(&c, "connectionEdges") {
            v if v.as_array().is_some() => {
                let mut edges = Vec::new();
                for q in v.as_array().map(|a| a.to_vec()).unwrap_or_default() {
                    edges.extend(query::evaluate(model, &q)?.into_iter().filter(|r| r.kind == EntityType::Edge));
                }
                edges
            }
            _ => Vec::new(),
        };
        for (e, t) in edges.into_iter().zip(parameters) {
            named.push(model.shape(e).edge_tangent(t, false).map_err(occt_error)?.origin);
        }
        let mut found: [Option<usize>; 2] = [None, None];
        for p in named {
            for (k, profile) in profiles.iter().enumerate() {
                let count = if profile.closed { profile.links.len() } else { profile.links.len() + 1 };
                if let Some(i) = (0..count).find(|&i| values::norm(values::sub(profile.vertex(i), p)) < 1e-7) {
                    found[k].get_or_insert(i);
                }
            }
        }
        let [Some(i), Some(j)] = found else { return Err(regen_error("LOFT_CONNECTION_MATCHING")) };
        out.push(Connection { vertices: [i, j] });
    }
    Ok(out)
}

/// The least total distance between matched vertices along a matching from `from` to `to`, and its pairs.
fn least_path(
    first: &Profile,
    second: &Profile,
    from: (usize, usize),
    to: (usize, usize),
) -> (f64, Vec<(usize, usize)>) {
    let distance = |i: usize, j: usize| values::norm(values::sub(first.vertex(i), second.vertex(j)));
    let (m, n) = (to.0 - from.0, to.1 - from.1);
    let mut cost = vec![vec![f64::INFINITY; n + 1]; m + 1];
    let mut back = vec![vec![(0, 0); n + 1]; m + 1];
    cost[0][0] = distance(from.0, from.1);
    for i in 0..=m {
        for j in 0..=n {
            for (di, dj) in [(1, 1), (1, 0), (0, 1)] {
                if i >= di && j >= dj && cost[i - di][j - dj] + distance(from.0 + i, from.1 + j) < cost[i][j] {
                    cost[i][j] = cost[i - di][j - dj] + distance(from.0 + i, from.1 + j);
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
    (cost[m][n], path.into_iter().map(|(i, j)| (from.0 + i, from.1 + j)).collect())
}

/// A closed profile's vertices' turn about `axis`: positive anticlockwise.
fn turn_about(profile: &Profile, axis: Vec3) -> f64 {
    let n = profile.links.len();
    let centre = values::scale(
        (0..n).fold(Vec3 { x: 0.0, y: 0.0, z: 0.0 }, |s, i| values::add(s, profile.vertex(i))),
        1.0 / n as f64,
    );
    (0..n)
        .map(|i| {
            values::dot(
                values::cross(values::sub(profile.vertex(i), centre), values::sub(profile.vertex(i + 1), centre)),
                axis,
            )
        })
        .sum()
}

/// A profile run the other way. A closed one keeps its first edge first, run backwards (batch 078 c005: a
/// rectangle's left side, from its second corner, where Onshape rips it).
fn reverse(profile: &mut Profile) {
    profile.links.reverse();
    for link in profile.links.iter_mut() {
        std::mem::swap(&mut link.from, &mut link.to);
        link.reversed = !link.reversed;
    }
    if profile.closed {
        profile.links.rotate_right(1);
    }
}

/// The two profiles and their vertices matched in order, in stretches between the connections asked for: index
/// pairs from (0, 0) to the two last, each step on by one along either or both, and the connections. An open second profile is turned to run the same way
/// as the first; closed ones both run anticlockwise about the way from the first to the second, so the sheet faces
/// out (batch 078 c005), the second starting where the matching is least (or both at the first connection). The
/// matching is the one with the least total distance between matched vertices through every connection, so an L
/// to a line fans its corner to the line's nearer end (batch 077 c001). Where a step along both runs between two
/// lines whose ends don't lie on a plane, the shorter diagonal is a match of its own (c003, 078 c004).
#[allow(clippy::type_complexity)]
fn matched(model: &Model, definition: &Value) -> R<(Profile, Profile, Vec<Vec<(usize, usize)>>, Vec<Connection>)> {
    let profiles = field(definition, "profileSubqueries")
        .as_array()
        .ok_or_else(|| error("@opTessellatedLoft: profileSubqueries must be an array"))?
        .clone();
    if profiles.len() != 2 {
        return Err(error("@opTessellatedLoft: profileSubqueries must hold two profiles"));
    }
    let mut first = profile_chain(model, &profiles[0])?;
    let mut second = profile_chain(model, &profiles[1])?;
    if first.closed != second.closed {
        return Err(error("@opTessellatedLoft: an open profile can't be lofted to a closed one"));
    }
    let (m, n) = (first.links.len(), second.links.len());
    if first.closed {
        let centre = |p: &Profile| {
            let k = p.links.len();
            values::scale(
                (0..k).fold(Vec3 { x: 0.0, y: 0.0, z: 0.0 }, |s, i| values::add(s, p.vertex(i))),
                1.0 / k as f64,
            )
        };
        let axis = values::sub(centre(&second), centre(&first));
        if turn_about(&first, axis) < 0.0 {
            reverse(&mut first);
        }
        if turn_about(&second, axis) < 0.0 {
            reverse(&mut second);
        }
    } else {
        let run = |p: &Profile| values::sub(p.vertex(p.links.len()), p.vertex(0));
        if values::dot(run(&first), run(&second)) < 0.0 {
            reverse(&mut second);
        }
    }
    let mut connections = connections(model, definition, [&first, &second])?;
    if first.closed {
        let (start1, start2) = match connections.first() {
            Some(c) => (c.vertices[0], c.vertices[1]),
            None => (
                0,
                (0..n)
                    .min_by(|&a, &b| {
                        let rotated = |k: usize| {
                            let mut s = Profile { links: Vec::new(), closed: true };
                            s.links.extend(second.links[k..].iter().chain(&second.links[..k]).map(|l| Link { ..*l }));
                            least_path(&first, &s, (0, 0), (m, n)).0
                        };
                        rotated(a).total_cmp(&rotated(b))
                    })
                    .unwrap_or(0),
            ),
        };
        first.links.rotate_left(start1);
        second.links.rotate_left(start2);
        for c in connections.iter_mut() {
            c.vertices = [(c.vertices[0] + m - start1) % m, (c.vertices[1] + n - start2) % n];
        }
    }
    // Through every connection, in order along both.
    let mut anchors: Vec<(usize, usize)> = connections.iter().map(|c| (c.vertices[0], c.vertices[1])).collect();
    anchors.push((0, 0));
    anchors.push((m, n));
    anchors.sort();
    anchors.dedup();
    if anchors.windows(2).any(|w| w[1].1 < w[0].1) {
        return Err(regen_error("LOFT_CONNECTION_MATCHING"));
    }
    let is_line = |link: &Link| model.shape(link.edge).curve_type().ok() == Some(occt::CurveType::Line);
    let mut stretches = Vec::new();
    for w in anchors.windows(2) {
        let path = least_path(&first, &second, w[0], w[1]).1;
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
        stretches.push(pairs);
    }
    Ok((first, second, stretches, connections))
}

/// The matched pairs of every stretch in turn, each once.
fn all_pairs(stretches: &[Vec<(usize, usize)>]) -> Vec<(usize, usize)> {
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for stretch in stretches {
        pairs.extend(stretch.iter().skip(usize::from(!pairs.is_empty())));
    }
    pairs
}

/// `@evTessellatedLoftMatches`: the profiles' vertices matched in order, stretch by stretch between connections
/// (batches 077, 078 c004). In a stretch a vertex is given on the edge starting there if it starts the stretch,
/// else on the edge ending there, at the edge's own parameter; or as itself where the stretch has no edge of that
/// profile. Each connection ends its stretch as a match hidden from view (std's rip edges skip it), then starts the
/// next.
pub fn matches(tags: &Tags, model: &Model, definition: &Value) -> R<Value> {
    let (first, second, stretches, connections) = matched(model, definition)?;
    let on_edge = |(p, e, t): (Vec3, EntityRef, f64)| {
        values::map(vec![
            ("position", tags.length_vector(p)),
            ("edge", tags.transient_query(&query::tid_string(model.tid(e)))),
            ("parameter", Value::number(t)),
        ])
    };
    let vertex = |profile: &Profile, i: usize| -> R<Value> {
        let p = profile.vertex(i);
        let edge = profile.links[i.min(profile.links.len() - 1)].edge;
        let body = &model.bodies[edge.body];
        let index = body
            .vertices
            .iter()
            .position(|v| v.shape.vertex_point().is_ok_and(|q| values::norm(values::sub(p, q)) < 1e-9))
            .ok_or_else(|| error("@evTessellatedLoftMatches: a profile's vertex has no vertex"))?;
        let v = EntityRef { body: edge.body, kind: EntityType::Vertex, index };
        Ok(values::map(vec![
            ("position", tags.length_vector(p)),
            ("vertex", tags.transient_query(&query::tid_string(model.tid(v)))),
        ]))
    };
    let item = |profile: &Profile, i: usize, from: usize, to: usize| -> R<Value> {
        if from == to { vertex(profile, i) } else { Ok(on_edge(profile.vertex_on_edge(i, i == from))) }
    };
    let pair = |(i, j): (usize, usize), from: (usize, usize), to: (usize, usize), hidden: bool| -> R<Value> {
        let items = vec![item(&first, i, from.0, to.0)?, item(&second, j, from.1, to.1)?];
        Ok(values::map(vec![("match", Value::array(items)), ("hideMatchFromUI", Value::bool(hidden))]))
    };
    let mut list = Vec::new();
    for (k, stretch) in stretches.iter().enumerate() {
        let (from, to) = (stretch[0], stretch[stretch.len() - 1]);
        let last = k + 1 == stretches.len();
        if k == 0 && connections.iter().any(|c| (c.vertices[0], c.vertices[1]) == from) {
            list.push(pair(from, from, to, true)?);
        }
        let shown = if last { &stretch[..] } else { &stretch[..stretch.len() - 1] };
        for &p in shown {
            if last && p == to && connections.iter().any(|c| (c.vertices[0], c.vertices[1]) == to) {
                list.push(pair(p, from, to, true)?);
            }
            list.push(pair(p, from, to, false)?);
        }
        if !last {
            list.push(pair(to, from, to, true)?);
        }
    }
    if first.closed {
        list.pop();
    }
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
    let (first, second, stretches, _) = matched(model, definition)?;
    let pairs = all_pairs(&stretches);
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
    let mut sheet =
        if faces.len() == 1 { faces.remove(0) } else { occt::sew_faces(&faces, 1e-7).map_err(occt_error)? };
    // Facets on one plane are one face: a connection's match between them makes no bend (batch 078 c004).
    let facets = sheet.sub_shapes(ShapeKind::Face);
    if facets.len() > 1 {
        let merged = occt::merge_faces(&facets).map_err(occt_error)?.shape().sub_shapes(ShapeKind::Face);
        if merged.len() < facets.len() {
            sheet = if merged.len() == 1 {
                merged[0].clone()
            } else {
                occt::sew_faces(&merged, 1e-7).map_err(occt_error)?
            };
        }
    }
    let info = Info::new(id);
    model.add_body(NewBody::new(sheet, BodyType::Sheet, info.clone()), &Origins::default(), &info, &|_| None);
    Ok(Value::UNDEFINED)
}
