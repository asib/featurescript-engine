//! Interpolating splines as Onshape makes them: cubic, with centripetal parameters normalized to [0, 1] and
//! natural ends unless end derivatives are given (worked out from fixture batch 004, c004–c006).

use crate::values;
use fs_interp::eval::{R, error};
use occt::{Shape, Vec3};

/// How a spline ends.
pub enum Ends {
    /// Zero second derivative at both ends.
    Natural,
    /// These first derivatives (with respect to the [0, 1] parameter) at the start and end.
    Clamped(Vec3, Vec3),
    /// This first derivative at the start; zero second derivative at the end.
    StartClamped(Vec3),
    /// Zero second derivative at the start; this first derivative at the end.
    EndClamped(Vec3),
    /// Closed and smooth all the way round; the last point isn't repeated.
    Periodic,
}

/// Centripetal parameters, normalized to [0, 1]: one per point, and one more for the closing span if periodic.
pub fn centripetal(points: &[Vec3], periodic: bool) -> Vec<f64> {
    let mut t = vec![0.0];
    let count = if periodic { points.len() } else { points.len() - 1 };
    for i in 0..count {
        let next = points[(i + 1) % points.len()];
        t.push(t[i] + values::norm(values::sub(next, points[i])).sqrt());
    }
    let total = *t.last().expect("at least one parameter");
    t.iter().map(|x| x / total).collect()
}

/// Solves a dense linear system by Gaussian elimination with partial pivoting.
pub(crate) fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for i in 0..n {
        let pivot = (i..n).max_by(|&p, &q| a[p][i].abs().total_cmp(&a[q][i].abs()))?;
        a.swap(i, pivot);
        b.swap(i, pivot);
        if a[i][i].abs() < 1e-300 {
            return None;
        }
        let pivot_row = a[i].clone();
        for (r, row) in a.iter_mut().enumerate() {
            if r != i {
                let f = row[i] / pivot_row[i];
                for (x, p) in row.iter_mut().zip(&pivot_row).skip(i) {
                    *x -= f * p;
                }
                b[r] -= f * b[i];
            }
        }
    }
    Some((0..n).map(|i| b[i] / a[i][i]).collect())
}

/// The first derivative at each point of the C2 cubic spline through `y` at parameters `t`.
fn derivatives_1d(t: &[f64], y: &[f64], ends: &Ends, d0: f64, d1: f64) -> Option<Vec<f64>> {
    let periodic = matches!(ends, Ends::Periodic);
    // Spans: n of them; for a periodic spline the last goes back to the first point.
    let n = if periodic { y.len() } else { y.len() - 1 };
    let value = |i: usize| y[i % y.len()];
    let h: Vec<f64> = (0..n).map(|i| t[i + 1] - t[i]).collect();
    let slope = |i: usize| (value(i + 1) - value(i)) / h[i];
    // Unknown second derivatives M_0..M_(m-1): m = points (periodic) or n + 1.
    let m = if periodic { n } else { n + 1 };
    let mut a = vec![vec![0.0; m]; m];
    let mut b = vec![0.0; m];
    let interior = if periodic { 0..n } else { 1..n };
    for i in interior {
        let prev = if i == 0 { n - 1 } else { i - 1 };
        a[i][prev % m] += h[prev];
        a[i][i] += 2.0 * (h[prev] + h[i]);
        a[i][(i + 1) % m] += h[i];
        b[i] = 6.0 * (slope(i) - slope(prev));
    }
    if !periodic {
        if matches!(ends, Ends::Clamped(..) | Ends::StartClamped(_)) {
            a[0][0] = 2.0 * h[0];
            a[0][1] = h[0];
            b[0] = 6.0 * (slope(0) - d0);
        } else {
            a[0][0] = 1.0;
        }
        if matches!(ends, Ends::Clamped(..) | Ends::EndClamped(_)) {
            a[n][n - 1] = h[n - 1];
            a[n][n] = 2.0 * h[n - 1];
            b[n] = 6.0 * (d1 - slope(n - 1));
        } else {
            a[n][n] = 1.0;
        }
    }
    let second = solve(a, b)?;
    let sd = |i: usize| second[i % m];
    let mut out: Vec<f64> = (0..n).map(|i| slope(i) - h[i] * (2.0 * sd(i) + sd(i + 1)) / 6.0).collect();
    if !periodic {
        out.push(slope(n - 1) + h[n - 1] * (sd(n - 1) + 2.0 * sd(n)) / 6.0);
    }
    Some(out)
}

/// A spline edge through points. `parameters`, if given, overrides the centripetal ones.
pub fn spline(points: &[Vec3], ends: Ends, parameters: Option<Vec<f64>>) -> R<Shape> {
    let periodic = matches!(ends, Ends::Periodic);
    if points.len() < 2 || (periodic && points.len() < 3) {
        return Err(error("a spline needs more points"));
    }
    let t = parameters.unwrap_or_else(|| centripetal(points, periodic));
    let zero = Vec3::new(0.0, 0.0, 0.0);
    let (d0, d1) = match &ends {
        Ends::Clamped(a, b) => (*a, *b),
        Ends::StartClamped(a) => (*a, zero),
        Ends::EndClamped(b) => (zero, *b),
        _ => (zero, zero),
    };
    let component = |f: fn(&Vec3) -> f64| -> R<Vec<f64>> {
        let y: Vec<f64> = points.iter().map(f).collect();
        derivatives_1d(&t, &y, &ends, f(&d0), f(&d1)).ok_or_else(|| error("spline: singular system"))
    };
    let (dx, dy, dz) = (component(|p| p.x)?, component(|p| p.y)?, component(|p| p.z)?);
    let derivatives: Vec<Vec3> = (0..points.len()).map(|i| Vec3::new(dx[i], dy[i], dz[i])).collect();
    occt::interpolated_edge(points, &derivatives, &t, periodic).map_err(|e| error(format!("spline: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The open spline of batch 004's c005: Onshape's region under it is 283.42663 mm², which these derivatives
    /// (centripetal parameters, natural ends) reproduce.
    #[test]
    fn natural_spline_derivatives() {
        let pts = [(0.0, 0.0), (10.0, 12.0), (25.0, 8.0), (35.0, 0.0)];
        let points: Vec<Vec3> = pts.iter().map(|(x, y)| Vec3::new(*x, *y, 0.0)).collect();
        let t = centripetal(&points, false);
        let y: Vec<f64> = pts.iter().map(|p| p.1).collect();
        let dy = derivatives_1d(&t, &y, &Ends::Natural, 0.0, 0.0).unwrap();
        assert!((t[1] - 0.3445472034952563).abs() < 1e-12);
        assert!((dy[0] - 46.29640655420955).abs() < 1e-9 && (dy[3] + 26.25405346894983).abs() < 1e-9);
    }
}
