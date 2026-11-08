//! Curves extended as Onshape extends them (worked out from fixture batch 036): as cubic B-splines, an arc first made
//! one, the extension added to the curve's end as more spans of the same B-spline.
//!
//! - **An arc** becomes the clamped cubic spline through points at equal angles along it, with its exact tangents
//!   (radius long) at the ends, parametrized by the angle from its start: as few spans, a power of two, as keep the
//!   spline's error bound (5/384 r θ⁴ for spans of θ) within 0.005 mm (r = 20: 11.25° spans; r = 10: 22.5°).
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

/// How far an arc made a B-spline may be from it, by the spline's error bound.
const ARC_TOLERANCE: f64 = 5e-6;
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
        solve(a.clone(), conditions.iter().map(|(_, _, v)| f(v)).collect()).ok_or_else(|| error("curve extension: singular system"))
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

    /// An arc about `centre` of `radius`, from `centre + radius x` turning `angle` towards `y`.
    pub fn arc(centre: Vec3, x: Vec3, y: Vec3, radius: f64, angle: f64) -> R<Cubic> {
        let mut n = 1usize;
        while 5.0 / 384.0 * radius * (angle / n as f64).powi(4) > ARC_TOLERANCE && n < 1 << 16 {
            n *= 2;
        }
        let h = angle / n as f64;
        let point = |t: f64| add(centre, add(scale(x, radius * t.cos()), scale(y, radius * t.sin())));
        let tangent = |t: f64| add(scale(x, -radius * t.sin()), scale(y, radius * t.cos()));
        let knots = [vec![0.0; 4], (1..n).map(|k| k as f64 * h).collect(), vec![angle; 4]].concat();
        let mut conditions: Vec<(f64, usize, Vec3)> = (0..=n).map(|k| (k as f64 * h, 0, point(k as f64 * h))).collect();
        conditions.push((0.0, 1, tangent(0.0)));
        conditions.push((angle, 1, tangent(angle)));
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
        let u = self.end();
        let (p0, p1, p2, p3) = (self.derivative(u, 0), self.derivative(u, 1), self.derivative(u, 2), self.derivative(u, 3));
        let t = normalize(p1).ok_or_else(|| error("curve extension: the curve stops at its end"))?;
        let s = dot(p2, t);
        let bend = norm(sub(p2, scale(t, s)));
        if bend < 1e-9 * norm(p1) * norm(p1) {
            return Ok(None);
        }
        // The blend's end tangent is p1 + Δ/2 p2; turned 3° from p1.
        let tan = BLEND_TURN.tan();
        let delta = 2.0 * norm(p1) * tan / (bend - s * tan);
        if !(delta > 0.0) {
            return Err(error("curve extension: no blend"));
        }
        let c = scale(add(p2, scale(p3, delta)), -1.0 / (6.0 * delta));
        let blend = |t: f64| {
            add(add(add(add(p0, scale(p1, t)), scale(p2, t * t / 2.0)), scale(p3, t * t * t / 6.0)), scale(c, t * t * t))
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
        const W: [f64; 5] = [0.5688888888888889, 0.47862867049936647, 0.47862867049936647, 0.23692688505618908, 0.23692688505618908];
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
