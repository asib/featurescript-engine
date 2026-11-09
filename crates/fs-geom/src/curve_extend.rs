//! Curves extended as Onshape extends them (worked out from fixture batch 036): as cubic B-splines, an arc first made
//! one, the extension added to the curve's end as more spans of the same B-spline.
//!
//! - **An arc or an elliptical arc** becomes the clamped cubic spline through points along it, with its exact
//!   tangents at the ends, parametrized by the angle (an arc's from its start, an ellipse's its own): its range is
//!   halved, and each half again, until for every span the curve's greatest distance from its centre times the
//!   span's angle to the fourth is at most 0.365 mm (batches 036, 038; between 0.357 and 0.376 by the ellipses). An
//!   arc's spans come out equal (r = 20: 11.25°; r = 10: 22.5°); a 20 × 10 ellipse's quarter has six of 11.25° and
//!   one of 22.5° at its flat end.
//! - **Straight on** (`TANGENT`), the extension is a straight span after a triple knot, as fast as the curve ends.
//! - **Curving on** (`CURVATURE`), a blend comes first: the last span's polynomial carried on with a cubic term
//!   added, so that it joins the curve with matching curvature (one knot) and ends with none, turned 3° from the
//!   curve's end; then the straight span along it. A curve ending without curvature (a fit spline's natural end)
//!   goes straight on.

use crate::bspline::{basis_derivatives, find_span};
use crate::splines::solve;
use crate::values::{add, cross, dot, norm, normalize, scale, sub};
use fs_interp::eval::{R, error};
use occt::{Shape, Vec3};

/// The most a span of an arc or ellipse made a B-spline may have of its greatest distance from the centre times its
/// angle to the fourth (m rad⁴).
const SPAN_BOUND: f64 = 0.365e-3;
/// How close a conic made a B-spline must lie to the conic at the middle of each span, at the same parameter (m):
/// this reproduces the knots of all 13 of Onshape's conic fillets in batches 003 and 052 (9 µm or 11 wouldn't).
const CONIC_TOLERANCE: f64 = 1e-5;
/// The same for a sketch's conic made a B-spline to extend it (batch 070 c003: a 20 mm conic of rho 0.7, its knots
/// from 1/32 to 1/8 apart; between 0.69 µm and 0.91 µm by them).
pub const CURVE_CONIC_TOLERANCE: f64 = 1e-6;
/// How far a curving extension's blend turns.
const BLEND_TURN: f64 = 3.0 * std::f64::consts::PI / 180.0;

/// A clamped, non-rational cubic B-spline: its full knot vector and its poles.
#[derive(Clone, Debug)]
pub struct Cubic {
    pub knots: Vec<f64>,
    pub poles: Vec<Vec3>,
}

fn row(knots: &[f64], count: usize, u: f64, order: usize) -> Vec<f64> {
    let span = find_span(count, 3, u, knots);
    let ders = basis_derivatives(span, u, 3, order, knots);
    let mut r = vec![0.0; count];
    for j in 0..=3 {
        r[span - 3 + j] = ders[order][j];
    }
    r
}

/// The poles of the cubic B-spline on `knots` meeting the conditions (parameter, derivative order, value), one per
/// pole.
fn fit(knots: &[f64], conditions: &[(f64, usize, Vec3)]) -> R<Vec<Vec3>> {
    let count = knots.len() - 4;
    if conditions.len() != count {
        return Err(error("curve extension: conditions and poles don't match"));
    }
    let a: Vec<Vec<f64>> = conditions.iter().map(|(u, order, _)| row(knots, count, *u, *order)).collect();
    let component = |f: fn(&Vec3) -> f64| -> R<Vec<f64>> {
        solve(a.clone(), conditions.iter().map(|(_, _, v)| f(v)).collect())
            .ok_or_else(|| error("curve extension: singular system"))
    };
    let (x, y, z) = (component(|v| v.x)?, component(|v| v.y)?, component(|v| v.z)?);
    Ok((0..count).map(|i| Vec3::new(x[i], y[i], z[i])).collect())
}

impl Cubic {
    /// The `order`th derivative at `u` (from the left at the end).
    pub fn derivative(&self, u: f64, order: usize) -> Vec3 {
        let r = row(&self.knots, self.poles.len(), u, order);
        r.iter().zip(&self.poles).fold(Vec3::new(0.0, 0.0, 0.0), |acc, (w, p)| add(acc, scale(*p, *w)))
    }

    pub fn end(&self) -> f64 {
        *self.knots.last().expect("knots")
    }

    pub fn edge(&self) -> R<Shape> {
        occt::bspline_edge(&self.poles, &[], &self.knots, 3, false).map_err(|e| error(format!("curve extension: {e}")))
    }

    /// An arc about `centre` of `radius`, from `centre + radius x` turning `angle` towards `y`, parametrized from 0.
    pub fn arc(centre: Vec3, x: Vec3, y: Vec3, radius: f64, angle: f64) -> R<Cubic> {
        Cubic::ellipse(centre, x, y, radius, radius, 0.0, angle)
    }

    /// The elliptical arc `centre + a cos t x + b sin t y` for t from `t0` to `t1`, parametrized by t.
    pub fn ellipse(centre: Vec3, x: Vec3, y: Vec3, a: f64, b: f64, t0: f64, t1: f64) -> R<Cubic> {
        let point = |t: f64| add(centre, add(scale(x, a * t.cos()), scale(y, b * t.sin())));
        let tangent = |t: f64| add(scale(x, -a * t.sin()), scale(y, b * t.cos()));
        let reach = |t: f64| (a * t.cos()).hypot(b * t.sin());
        // The greatest distance from the centre over a span: at an end, or where the major axis crosses it.
        let greatest = |lo: f64, hi: f64| {
            let mut m = reach(lo).max(reach(hi));
            let pi = std::f64::consts::PI;
            let mut k = (lo / pi).ceil();
            while k * pi < hi {
                m = m.max(reach(k * pi));
                k += 1.0;
            }
            m
        };
        let mut knots_inner = Vec::new();
        let mut pending = vec![(t0, t1)];
        while let Some((lo, hi)) = pending.pop() {
            if greatest(lo, hi) * (hi - lo).powi(4) <= SPAN_BOUND || hi - lo < 1e-6 {
                continue;
            }
            let mid = (lo + hi) / 2.0;
            knots_inner.push(mid);
            pending.push((lo, mid));
            pending.push((mid, hi));
        }
        knots_inner.sort_by(f64::total_cmp);
        let knots = [vec![t0; 4], knots_inner.clone(), vec![t1; 4]].concat();
        let mut conditions: Vec<(f64, usize, Vec3)> = std::iter::once(t0)
            .chain(knots_inner.iter().copied())
            .chain(std::iter::once(t1))
            .map(|t| (t, 0, point(t)))
            .collect();
        conditions.push((t0, 1, tangent(t0)));
        conditions.push((t1, 1, tangent(t1)));
        let poles = fit(&knots, &conditions)?;
        Ok(Cubic { knots, poles })
    }

    /// The conic arc from `p0` to `p2` with control point `p1` and middle weight `w` (the rational quadratic Bézier
    /// `(b0 p0 + w b1 p1 + b2 p2) / (b0 + w b1 + b2)`), as Onshape makes a conic fillet's cross section (batches
    /// 003, 052): the clamped cubic spline through it at its knots, in its own parameter (0 to 1), with its exact
    /// derivatives at the ends. The knots start at the ends alone; each round, every span whose middle lies more
    /// than [`CONIC_TOLERANCE`] from the conic at the same parameter is halved, and the spline fitted again.
    pub fn rational_quadratic(p0: Vec3, p1: Vec3, p2: Vec3, w: f64) -> R<Cubic> {
        Cubic::rational_quadratic_within(p0, p1, p2, w, CONIC_TOLERANCE)
    }

    /// [`Cubic::rational_quadratic`] to within `tolerance` (m) at the middle of each span.
    pub fn rational_quadratic_within(p0: Vec3, p1: Vec3, p2: Vec3, w: f64, tolerance: f64) -> R<Cubic> {
        let point = |t: f64| {
            let (b0, b1, b2) = ((1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t) * w, t * t);
            scale(add(add(scale(p0, b0), scale(p1, b1)), scale(p2, b2)), 1.0 / (b0 + b1 + b2))
        };
        // d/dt of N/D is (N' D - N D') / D².
        let tangent = |t: f64| {
            let (b0, b1, b2) = ((1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t) * w, t * t);
            let (d0, d1, d2) = (-2.0 * (1.0 - t), 2.0 * (1.0 - 2.0 * t) * w, 2.0 * t);
            let (n, dn) = (
                add(add(scale(p0, b0), scale(p1, b1)), scale(p2, b2)),
                add(add(scale(p0, d0), scale(p1, d1)), scale(p2, d2)),
            );
            let (d, dd) = (b0 + b1 + b2, d0 + d1 + d2);
            scale(sub(scale(dn, d), scale(n, dd)), 1.0 / (d * d))
        };
        let mut parameters = vec![0.0, 1.0];
        loop {
            let points: Vec<Vec3> = parameters.iter().map(|t| point(*t)).collect();
            let cubic = Cubic::interpolate(&parameters, &points, tangent(0.0), tangent(1.0))?;
            let halves: Vec<f64> = parameters
                .windows(2)
                .map(|span| (span[0] + span[1]) / 2.0)
                .filter(|&m| norm(sub(cubic.derivative(m, 0), point(m))) > tolerance)
                .collect();
            if halves.is_empty() || parameters.len() > 1024 {
                return Ok(cubic);
            }
            parameters.extend(halves);
            parameters.sort_by(f64::total_cmp);
        }
    }

    /// The clamped cubic spline through `points` at increasing `parameters` (its knots), with the derivatives `start`
    /// and `end` there.
    pub fn interpolate(parameters: &[f64], points: &[Vec3], start: Vec3, end: Vec3) -> R<Cubic> {
        let n = parameters.len();
        if n < 2 || points.len() != n {
            return Err(error("interpolation: needs as many points as parameters, two at least"));
        }
        let knots = [vec![parameters[0]; 4], parameters[1..n - 1].to_vec(), vec![parameters[n - 1]; 4]].concat();
        let mut conditions: Vec<(f64, usize, Vec3)> = parameters.iter().zip(points).map(|(t, p)| (*t, 0, *p)).collect();
        conditions.push((parameters[0], 1, start));
        conditions.push((parameters[n - 1], 1, end));
        let poles = fit(&knots, &conditions)?;
        Ok(Cubic { knots, poles })
    }

    /// The direction and speed at the end.
    fn end_tangent(&self) -> R<(Vec3, f64)> {
        let d = self.derivative(self.end(), 1);
        let speed = norm(d);
        Ok((normalize(d).ok_or_else(|| error("curve extension: the curve stops at its end"))?, speed))
    }

    /// This curve carried straight on from its end for `length`.
    pub fn straight_on(&self, length: f64) -> R<Cubic> {
        let (direction, speed) = self.end_tangent()?;
        let end = *self.poles.last().expect("poles");
        let mut knots = self.knots[..self.knots.len() - 1].to_vec();
        knots.extend(std::iter::repeat_n(self.end() + length / speed, 4));
        let mut poles = self.poles.clone();
        poles.extend((1..=3).map(|k| add(end, scale(direction, length * k as f64 / 3.0))));
        Ok(Cubic { knots, poles })
    }

    /// This curve with the blend a curving extension starts with, or `None` if the curve ends without curvature.
    pub fn blended(&self) -> R<Option<Cubic>> {
        self.blended_to(None)
    }

    /// [`Cubic::blended`], or with the blend cut short `cut` past the curve's end (in its parameter): an extension
    /// shorter than its blend is the blend as far as it goes (batch 070 c004).
    ///
    /// The blend runs on for at most the curve's own parameter range: a conic's end, made a B-spline over 0 to 1,
    /// would turn its 3° only 1.57 on, and Onshape's blend there (batch 070 c004) is the one 1 on. (One case shows
    /// it; the arcs of batches 036 and 038 turn their 3° well within their ranges.)
    pub fn blended_to(&self, cut: Option<f64>) -> R<Option<Cubic>> {
        let u = self.end();
        let (p0, p1, p2, p3) =
            (self.derivative(u, 0), self.derivative(u, 1), self.derivative(u, 2), self.derivative(u, 3));
        let t = normalize(p1).ok_or_else(|| error("curve extension: the curve stops at its end"))?;
        let s = dot(p2, t);
        let bend = norm(sub(p2, scale(t, s)));
        if bend < 1e-9 * norm(p1) * norm(p1) {
            return Ok(None);
        }
        // The blend's end tangent is p1 + Δ/2 p2; turned 3° from p1.
        let tan = BLEND_TURN.tan();
        let mut delta = 2.0 * norm(p1) * tan / (bend - s * tan);
        let range = u - self.start();
        if !(delta > 0.0) || delta > range {
            delta = range;
        }
        let c = scale(add(p2, scale(p3, delta)), -1.0 / (6.0 * delta));
        let delta_full = delta;
        let delta = cut.map_or(delta_full, |d| d.min(delta_full));
        let blend = |t: f64| {
            add(
                add(add(add(p0, scale(p1, t)), scale(p2, t * t / 2.0)), scale(p3, t * t * t / 6.0)),
                scale(c, t * t * t),
            )
        };
        let mut knots = self.knots[..self.knots.len() - 3].to_vec();
        knots.extend(std::iter::repeat_n(u + delta, 4));
        // The curve and blend together lie in the new knots' space: interpolated at its Greville points, exactly.
        let count = knots.len() - 4;
        let conditions: Vec<(f64, usize, Vec3)> = (0..count)
            .map(|i| {
                let g = (knots[i + 1] + knots[i + 2] + knots[i + 3]) / 3.0;
                (g, 0, if g <= u { self.derivative(g, 0) } else { blend(g - u) })
            })
            .collect();
        let poles = fit(&knots, &conditions)?;
        Ok(Some(Cubic { knots, poles }))
    }

    /// The length of the curve from parameter `a` to `b`.
    pub fn length(&self, a: f64, b: f64) -> f64 {
        const X: [f64; 5] = [0.0, -0.5384693101056831, 0.5384693101056831, -0.906179845938664, 0.906179845938664];
        const W: [f64; 5] =
            [0.5688888888888889, 0.47862867049936647, 0.47862867049936647, 0.23692688505618908, 0.23692688505618908];
        let pieces = 64;
        let h = (b - a) / pieces as f64;
        (0..pieces)
            .map(|k| {
                let mid = a + h * (k as f64 + 0.5);
                X.iter().zip(W).map(|(x, w)| w * norm(self.derivative(mid + x * h / 2.0, 1))).sum::<f64>() * h / 2.0
            })
            .sum()
    }

    /// The end and the direction there.
    pub fn end_ray(&self) -> R<(Vec3, Vec3)> {
        Ok((*self.poles.last().expect("poles"), self.end_tangent()?.0))
    }

    pub fn start(&self) -> f64 {
        self.knots[0]
    }

    /// The same curve run the other way, its parameter negated (so that extending it at its end and reversing back
    /// extends this one at its start, its own parameters kept, as Onshape does, batch 038).
    pub fn reversed(&self) -> Cubic {
        Cubic {
            knots: self.knots.iter().rev().map(|k| -k).collect(),
            poles: self.poles.iter().rev().copied().collect(),
        }
    }

    /// The part of this curve between parameters `a` and `b`, as the same B-spline restricted (its parameters kept).
    pub fn segment(&self, a: f64, b: f64) -> R<Cubic> {
        let inner: Vec<f64> = self.knots.iter().copied().filter(|k| *k > a + 1e-12 && *k < b - 1e-12).collect();
        let knots = [vec![a; 4], inner, vec![b; 4]].concat();
        let count = knots.len() - 4;
        let conditions: Vec<(f64, usize, Vec3)> = (0..count)
            .map(|i| {
                let g = (knots[i + 1] + knots[i + 2] + knots[i + 3]) / 3.0;
                (g, 0, self.derivative(g, 0))
            })
            .collect();
        let poles = fit(&knots, &conditions)?;
        Ok(Cubic { knots, poles })
    }

    /// The parameters where the curve crosses a plane.
    pub fn plane_crossings(&self, origin: Vec3, normal: Vec3) -> Vec<f64> {
        let f = |u: f64| dot(sub(self.derivative(u, 0), origin), normal);
        let (a, b) = (self.start(), self.end());
        let steps = 64 * self.poles.len();
        let mut out = Vec::new();
        let mut prev = (a, f(a));
        for k in 1..=steps {
            let u = a + (b - a) * k as f64 / steps as f64;
            let fu = f(u);
            if prev.1 == 0.0 {
                out.push(prev.0);
            } else if prev.1 * fu < 0.0 {
                let (mut lo, mut hi, mut flo) = (prev.0, u, prev.1);
                for _ in 0..200 {
                    let mid = (lo + hi) / 2.0;
                    let fm = f(mid);
                    if fm * flo <= 0.0 {
                        hi = mid;
                    } else {
                        lo = mid;
                        flo = fm;
                    }
                }
                out.push((lo + hi) / 2.0);
            }
            prev = (u, fu);
        }
        out
    }

    /// The normal of the plane the curve lies in, if it lies in one.
    pub fn plane_normal(&self) -> Option<Vec3> {
        let first = self.poles[0];
        let mut normal = None;
        for i in 1..self.poles.len() {
            for j in i + 1..self.poles.len() {
                if let Some(n) = normalize(cross(sub(self.poles[i], first), sub(self.poles[j], first))) {
                    normal = Some(n);
                    break;
                }
            }
            if normal.is_some() {
                break;
            }
        }
        let n = normal?;
        self.poles.iter().all(|p| dot(sub(*p, first), n).abs() < 1e-9).then_some(n)
    }
}

/// The frame of an arc from `start` (leaving along `tangent`) to `end` about `centre`: its x and y axes and the
/// angle it turns.
pub fn arc_frame(centre: Vec3, start: Vec3, tangent: Vec3, end: Vec3) -> R<(Vec3, Vec3, f64, f64)> {
    let radial = sub(start, centre);
    let radius = norm(radial);
    let x = normalize(radial).ok_or_else(|| error("arc: no radius"))?;
    let normal = normalize(cross(radial, tangent)).ok_or_else(|| error("arc: no plane"))?;
    let y = cross(normal, x);
    let e = sub(end, centre);
    let mut angle = dot(e, y).atan2(dot(e, x));
    if angle <= 1e-12 {
        angle += 2.0 * std::f64::consts::PI;
    }
    Ok((x, y, radius, angle))
}
