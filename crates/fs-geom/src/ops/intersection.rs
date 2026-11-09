//! Where Onshape splits the curves two sets of faces meet in (batch 060): each closed curve starts on the seam of
//! the cylinder it winds round (the second group's first, then the first's), and is split wherever it touches an
//! edge bounding either face. The curves run along n₁ × n₂, the first group's face normal crossed with the
//! second's, and their pieces come from the seam on its frame's −y side round.

use crate::model::Model;
use crate::values::{cross, dot, norm, scale, sub};
use occt::{Frame, Shape, ShapeKind, Vec3};

const SAMPLES: usize = 512;

/// The coordinate system std sees on a cylinder face (`evSurfaceDefinition`), if it is one.
fn cylinder_frame(model: &Model, face: &Shape) -> Option<Frame> {
    let s = face.surface_definition().ok()?;
    if s.kind != 1 {
        return None;
    }
    let known = model.bodies.iter().flat_map(|b| b.faces.iter()).find(|f| f.shape.is_same(face));
    Some(known.and_then(|f| f.info.surface_frame).unwrap_or(s.frame))
}

/// The points of an edge at evenly spaced fractions of its length, the last one its end.
fn sample(edge: &Shape) -> Option<Vec<(f64, Vec3)>> {
    (0..=SAMPLES)
        .map(|i| {
            let t = i as f64 / SAMPLES as f64;
            edge.edge_tangent(t, false).ok().map(|a| (t, a.origin))
        })
        .collect()
}

/// The fraction (by length) of the point between fractions `a` and `b` of an edge that minimises `f`, by golden
/// section.
fn minimise(edge: &Shape, mut a: f64, mut b: f64, f: impl Fn(Vec3) -> f64) -> Option<(f64, f64)> {
    let g = (5f64.sqrt() - 1.0) / 2.0;
    let at = |t: f64| edge.edge_tangent(t.clamp(0.0, 1.0), false).ok().map(|x| f(x.origin));
    for _ in 0..60 {
        let (c, d) = (b - g * (b - a), a + g * (b - a));
        if at(c)? < at(d)? {
            b = d;
        } else {
            a = c;
        }
    }
    let t = (a + b) / 2.0;
    Some((t, at(t)?))
}

/// The fractions along `edge` where it touches the boundary edges of `faces` (seams aside), away from its ends if
/// it is open.
fn touches(edge: &Shape, points: &[(f64, Vec3)], faces: &[Shape], closed: bool) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for face in faces {
        for boundary in face.sub_shapes(ShapeKind::Edge) {
            if face.is_seam(&boundary) || edge.distance(&boundary).map_or(true, |d| d.distance > 1e-6) {
                continue;
            }
            let distance = |p: Vec3| occt::vertex(p).and_then(|v| v.distance(&boundary)).map_or(f64::MAX, |d| d.distance);
            let d: Vec<f64> = points.iter().map(|(_, p)| distance(*p)).collect();
            for i in 0..d.len() {
                let (before, after) = match (i, closed) {
                    (0, true) => (d[d.len() - 2], d[1]),
                    (0, false) => continue,
                    (i, _) if i == d.len() - 1 => continue,
                    (i, _) => (d[i - 1], d[i + 1]),
                };
                if d[i] > 1e-4 || d[i] > before || d[i] > after {
                    continue;
                }
                let (a, b) = (points[i].0 - 1.0 / SAMPLES as f64, points[i].0 + 1.0 / SAMPLES as f64);
                let Some((t, min)) = minimise(edge, a, b, distance) else { continue };
                let t = t.rem_euclid(1.0);
                let near_end = !closed && (t < 1e-6 || t > 1.0 - 1e-6);
                if min < 1e-6 && !near_end && !out.iter().any(|&u| (u - t).abs() < 1e-6 || (u - t).abs() > 1.0 - 1e-6) {
                    out.push(t);
                }
            }
        }
    }
    out
}

/// The angle of a point about a frame's axis, from its x axis towards its y.
fn angle(frame: &Frame, p: Vec3) -> f64 {
    let r = sub(p, frame.origin);
    let y = cross(frame.normal, frame.x);
    dot(r, y).atan2(dot(r, frame.x))
}

/// Where a closed curve crosses a cylinder's seam (its frame's x side), if it crosses it once: the fraction along
/// the curve, and whether the curve runs towards the frame's +y there.
fn seam_crossing(edge: &Shape, points: &[(f64, Vec3)], frame: &Frame) -> Option<(f64, bool)> {
    let y = cross(frame.normal, frame.x);
    let mut crossings = Vec::new();
    // Round the closed curve, its start once (its end is the same point).
    let n = points.len() - 1;
    for i in 0..n {
        let w = [points[i], (if i + 1 == n { 1.0 } else { points[i + 1].0 }, points[(i + 1) % n].1)];
        let (ra, rb) = (sub(w[0].1, frame.origin), sub(w[1].1, frame.origin));
        let (va, vb) = (dot(ra, y), dot(rb, y));
        if dot(ra, frame.x) > 0.0 && dot(rb, frame.x) > 0.0 && (va < 0.0) != (vb < 0.0) {
            crossings.push((w[0].0, w[1].0, vb > va));
        }
    }
    let [(a, b, rising)] = crossings[..] else { return None };
    let (t, _) = minimise(edge, a, b, |p| angle(frame, p).abs())?;
    Some((t, rising))
}

/// An edge refitted to be parametrised by its length, as Onshape's intersection curves nearly are (their middle
/// by parameter is within 0.02 mm of the middle by length, batch 060).
fn by_length(edge: &Shape) -> Option<Shape> {
    const SPANS: usize = 64;
    let length = edge.length().ok()?;
    let mut points = Vec::new();
    let mut derivatives = Vec::new();
    let mut parameters = Vec::new();
    for i in 0..=SPANS {
        let t = i as f64 / SPANS as f64;
        let a = edge.edge_tangent(t, true).ok()?;
        points.push(a.origin);
        derivatives.push(a.direction);
        parameters.push(t * length);
    }
    occt::interpolated_edge(&points, &derivatives, &parameters, false).ok()
}

/// The face of `faces` a point lies on, and its normal there.
fn normal_at(faces: &[Shape], p: Vec3) -> Option<Vec3> {
    let v = occt::vertex(p).ok()?;
    let face = faces.iter().find(|f| v.distance(f).is_ok_and(|d| d.distance < 1e-6))?;
    let (fu, fv) = face.face_fraction(p).ok()?;
    Some(face.face_tangent_plane(fu, fv).ok()?.normal)
}

/// A curve `tools` and `targets` meet in, as Onshape splits it: its pieces in order and a key to order the curves
/// by, or `None` to leave it whole.
pub(crate) fn split_curve(model: &Model, edge: &Shape, tools: &[Shape], targets: &[Shape]) -> Option<(Vec<Shape>, f64)> {
    let ends = edge.edge_end_vertices().ok()?;
    let closed = match &ends[..] {
        [a, b] => norm(sub(a.vertex_point().ok()?, b.vertex_point().ok()?)) < 1e-7,
        _ => true,
    };
    let points = sample(edge)?;
    // Run along n₁ × n₂.
    let probe = edge.edge_tangent(0.25, false).ok()?;
    let along = cross(normal_at(tools, probe.origin)?, normal_at(targets, probe.origin)?);
    let forward = dot(along, probe.direction) >= 0.0;
    let mut cuts = touches(edge, &points, tools, closed);
    for t in touches(edge, &points, targets, closed) {
        if !cuts.iter().any(|&u| (u - t).abs() < 1e-6) {
            cuts.push(t);
        }
    }
    let frames: Vec<Frame> =
        targets.iter().chain(tools.iter()).filter_map(|f| cylinder_frame(model, f)).collect();
    let seam = if closed { frames.iter().find_map(|f| seam_crossing(edge, &points, f).map(|s| (s, *f))) } else { None };
    if let Some(((t, _), _)) = seam
        && !cuts.iter().any(|&u| (u - t).abs() < 1e-6 || (u - t).abs() > 1.0 - 1e-6)
    {
        cuts.push(t);
    }
    // The curves are ordered along the first frame's axis, against its z (along an extrude).
    let key = frames.first().map_or(0.0, |f| {
        let mean = points.iter().fold([0.0; 3], |m, (_, p)| [m[0] + p.x, m[1] + p.y, m[2] + p.z]);
        let mean = Vec3 { x: mean[0], y: mean[1], z: mean[2] };
        -dot(scale(mean, 1.0 / points.len() as f64), f.normal)
    });
    if cuts.is_empty() {
        return None;
    }
    cuts.sort_by(f64::total_cmp);
    // The pieces between the cuts, along the edge.
    let mut pieces: Vec<(f64, f64)> = cuts.windows(2).map(|w| (w[0], w[1])).collect();
    if closed {
        pieces.push((*cuts.last()?, cuts[0]));
    } else {
        pieces.insert(0, (0.0, cuts[0]));
        pieces.push((*cuts.last()?, 1.0));
    }
    // Start from the seam, on its −y side, and go round away from it.
    if let Some(((t, rising), _)) = seam {
        let at_seam = |u: f64| (u - t).abs() < 1e-6 || (u - t).abs() > 1.0 - 1e-6;
        // Along the edge, the piece ending at the seam is on its −y side if the edge rises through it.
        let first = pieces.iter().position(|&(a, b)| if rising { at_seam(b) } else { at_seam(a) })?;
        pieces.rotate_left(first);
        if rising {
            // Going round against the edge: keep the first, then the rest backwards.
            pieces[1..].reverse();
        }
    }
    let shapes = pieces
        .into_iter()
        .map(|(a, b)| occt::edge_between(edge, a, b, !forward).ok().map(|e| by_length(&e).unwrap_or(e)))
        .collect::<Option<Vec<_>>>()?;
    Some((shapes, key))
}
