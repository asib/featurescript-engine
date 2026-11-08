//! B-spline curves as std's `BSplineCurve`, and the builtins that make them: `@evApproximateBSplineCurve` (an
//! edge's curve as a B-spline), `@approximateSpline` (a least-squares fit to points within a tolerance) and
//! `@evMaxPathDeviation` (how far apart two paths get); and fitting a surface to points (`@opConstrainedSurface`).

use crate::model::{EntityType, Model};
use crate::splines::solve;
use crate::values::{self, Tags, field, map, norm, sub};
use crate::{GeomBuiltins, model_mut, occt_error, query, regen_error, tags};
use fs_interp::Value;
use fs_interp::builtins::arg;
use fs_interp::eval::{R, error};
use occt::{BSplineData, Shape, Vec3};

/// A B-spline as std's `BSplineCurve`: a periodic one's control points overlap by `degree`.
pub fn bspline_value(tags: &Tags, data: &BSplineData) -> R<Value> {
    let mut poles = data.poles.clone();
    let mut weights = data.weights.clone();
    if data.periodic {
        let d = data.degree as usize;
        poles.extend(data.poles.iter().take(d).copied());
        weights.extend(data.weights.iter().take(d).copied());
    }
    let numbers = |xs: &[f64]| Value::array(xs.iter().map(|&x| Value::number(x)).collect());
    let mut entries = vec![
        ("degree", Value::number(data.degree as f64)),
        ("dimension", Value::number(3.0)),
        ("isRational", Value::bool(data.rational)),
        ("isPeriodic", Value::bool(data.periodic)),
        ("controlPoints", Value::array(poles.iter().map(|&p| tags.length_vector(p)).collect())),
        ("knots", tags.tagged(numbers(&data.knots), "KnotArray")?),
    ];
    if data.rational {
        entries.push(("weights", numbers(&weights)));
    }
    tags.tagged(map(entries), "BSplineCurve")
}

// ----- B-spline basics (Piegl and Tiller, The NURBS Book, chapter 2) -----

/// The knot span holding `u` for `n` control points of degree `p`.
fn find_span(n: usize, p: usize, u: f64, knots: &[f64]) -> usize {
    if u >= knots[n] {
        return n - 1;
    }
    if u <= knots[p] {
        return p;
    }
    let (mut low, mut high) = (p, n);
    let mut mid = (low + high) / 2;
    while u < knots[mid] || u >= knots[mid + 1] {
        if u < knots[mid] {
            high = mid
        } else {
            low = mid
        }
        mid = (low + high) / 2;
    }
    mid
}

/// The basis functions that aren't zero at `u` in `span`, and their derivatives up to `order`: `out[k][j]` is
/// the k-th derivative of N_{span - p + j}.
fn basis_derivatives(span: usize, u: f64, p: usize, order: usize, knots: &[f64]) -> Vec<Vec<f64>> {
    let mut ndu = vec![vec![0.0; p + 1]; p + 1];
    let (mut left, mut right) = (vec![0.0; p + 1], vec![0.0; p + 1]);
    ndu[0][0] = 1.0;
    for j in 1..=p {
        left[j] = u - knots[span + 1 - j];
        right[j] = knots[span + j] - u;
        let mut saved = 0.0;
        for r in 0..j {
            ndu[j][r] = right[r + 1] + left[j - r];
            let temp = if ndu[j][r] == 0.0 { 0.0 } else { ndu[r][j - 1] / ndu[j][r] };
            ndu[r][j] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        ndu[j][j] = saved;
    }
    let order = order.min(p);
    let mut out = vec![vec![0.0; p + 1]; order + 1];
    for j in 0..=p {
        out[0][j] = ndu[j][p];
    }
    let mut a = vec![vec![0.0; p + 1]; 2];
    for r in 0..=p {
        let (mut s1, mut s2) = (0usize, 1usize);
        a[0][0] = 1.0;
        for k in 1..=order {
            let mut d = 0.0;
            let rk = r as isize - k as isize;
            let pk = p - k;
            if rk >= 0 {
                let rk = rk as usize;
                a[s2][0] = if ndu[pk + 1][rk] == 0.0 { 0.0 } else { a[s1][0] / ndu[pk + 1][rk] };
                d = a[s2][0] * ndu[rk][pk];
            }
            let j1 = if rk >= -1 { 1 } else { (-rk) as usize };
            let j2 = if (r as isize - 1) <= pk as isize { k - 1 } else { p - r };
            for j in j1..=j2 {
                let idx = (rk + j as isize) as usize;
                a[s2][j] = if ndu[pk + 1][idx] == 0.0 { 0.0 } else { (a[s1][j] - a[s1][j - 1]) / ndu[pk + 1][idx] };
                d += a[s2][j] * ndu[idx][pk];
            }
            if r <= pk {
                a[s2][k] = if ndu[pk + 1][r] == 0.0 { 0.0 } else { -a[s1][k - 1] / ndu[pk + 1][r] };
                d += a[s2][k] * ndu[r][pk];
            }
            out[k][r] = d;
            std::mem::swap(&mut s1, &mut s2);
        }
    }
    // Row k is scaled by p! / (p − k)!.
    let mut factor = p as f64;
    for (k, row) in out.iter_mut().enumerate().skip(1) {
        for x in row.iter_mut() {
            *x *= factor;
        }
        factor *= (p - k) as f64;
    }
    out
}

fn axpy(acc: Vec3, a: f64, p: Vec3) -> Vec3 {
    Vec3::new(acc.x + a * p.x, acc.y + a * p.y, acc.z + a * p.z)
}

/// A non-rational, clamped B-spline.
struct Spline {
    degree: usize,
    knots: Vec<f64>,
    poles: Vec<Vec3>,
}

impl Spline {
    /// The point and its first two derivatives at `u`.
    fn eval(&self, u: f64) -> [Vec3; 3] {
        let p = self.degree;
        let span = find_span(self.poles.len(), p, u, &self.knots);
        let ders = basis_derivatives(span, u, p, 2, &self.knots);
        let mut out = [Vec3::new(0.0, 0.0, 0.0); 3];
        for (k, row) in ders.iter().enumerate() {
            for (j, n) in row.iter().enumerate() {
                out[k] = axpy(out[k], *n, self.poles[span - p + j]);
            }
        }
        out
    }

    /// The parameter near `u` whose point is closest to `q`, by a few Newton steps.
    fn project(&self, q: Vec3, mut u: f64) -> f64 {
        let (a, b) = (self.knots[self.degree], self.knots[self.poles.len()]);
        for _ in 0..4 {
            let [c, d1, d2] = self.eval(u);
            let r = sub(c, q);
            let f = values::dot(r, d1);
            let df = values::dot(d1, d1) + values::dot(r, d2);
            if df.abs() < 1e-300 {
                break;
            }
            u = (u - f / df).clamp(a, b);
        }
        u
    }
}

// ----- least-squares approximation -----

/// One family member to approximate: points, and derivatives at the ends.
struct Target {
    points: Vec<Vec3>,
    start: [Option<Vec3>; 2],
    end: [Option<Vec3>; 2],
}

/// Linear constraints on the unknowns, rows of `coefficients · x = value`.
#[derive(Default)]
struct Constraints {
    rows: Vec<(Vec<(usize, f64)>, f64)>,
}

impl Constraints {
    fn push(&mut self, terms: Vec<(usize, f64)>, value: f64) {
        self.rows.push((terms, value));
    }
}

/// Fits poles of degree `p` on `knots` to `points` at `params`, keeping the constraints that apply. Unknowns are
/// the poles' coordinates, then one scale for each end derivative given only as a direction.
fn fit(
    p: usize,
    knots: &[f64],
    params: &[f64],
    target: &Target,
    interpolate: &[usize],
    derivative_magnitudes: bool,
) -> Option<Vec<Vec3>> {
    let n = knots.len() - p - 1;
    let points = &target.points;
    let m = points.len();
    let free_scales: Vec<bool> =
        [target.start[0], target.end[0]].iter().map(|d| d.is_some() && !derivative_magnitudes).collect();
    let scale_index = |which: usize| 3 * n + free_scales[..which].iter().filter(|&&f| f).count();
    let unknowns = 3 * n + free_scales.iter().filter(|&&f| f).count();
    let mut normal = vec![vec![0.0; unknowns]; unknowns];
    let mut rhs = vec![0.0; unknowns];
    let mut basis_at = Vec::with_capacity(m);
    for (k, &u) in params.iter().enumerate() {
        let span = find_span(n, p, u, knots);
        let basis = basis_derivatives(span, u, p, 0, knots).swap_remove(0);
        for (a, &na) in basis.iter().enumerate() {
            let i = span - p + a;
            for (b, &nb) in basis.iter().enumerate() {
                let j = span - p + b;
                for c in 0..3 {
                    normal[3 * i + c][3 * j + c] += na * nb;
                }
            }
            let q = [points[k].x, points[k].y, points[k].z];
            for c in 0..3 {
                rhs[3 * i + c] += na * q[c];
            }
        }
        basis_at.push((span, basis));
    }
    // A little smoothing keeps poles no point pins down from wandering off.
    let scale = normal.iter().enumerate().map(|(i, row)| row[i]).fold(0.0f64, f64::max).max(1.0);
    let smooth = 1e-10 * scale;
    for i in 0..n.saturating_sub(1) {
        for c in 0..3 {
            let (a, b) = (3 * i + c, 3 * (i + 1) + c);
            normal[a][a] += smooth;
            normal[b][b] += smooth;
            normal[a][b] -= smooth;
            normal[b][a] -= smooth;
        }
    }
    let mut constraints = Constraints::default();
    let pin = |constraints: &mut Constraints, k: usize| {
        let (span, basis) = &basis_at[k];
        for (c, q) in [points[k].x, points[k].y, points[k].z].into_iter().enumerate() {
            constraints.push(basis.iter().enumerate().map(|(a, &na)| (3 * (span - p + a) + c, na)).collect(), q);
        }
    };
    let mut pinned: Vec<usize> = vec![0, m - 1];
    pinned.extend(interpolate.iter().copied().filter(|&k| k > 0 && k < m - 1));
    pinned.sort_unstable();
    pinned.dedup();
    for &k in &pinned {
        pin(&mut constraints, k);
    }
    let (a, b) = (knots[p], knots[n]);
    // Derivatives at the start: C'(a) = p (P1 − P0) / (U[p+1] − a); at the end, p (P[n−1] − P[n−2]) / (b − U[n−1]).
    for (which, (first, second, sign)) in [(0usize, 1usize, 1.0), (n - 1, n - 2, -1.0)].into_iter().enumerate() {
        let ders = if which == 0 { target.start } else { target.end };
        let Some(d1) = ders[0] else { continue };
        let h1 = if which == 0 { knots[p + 1] - a } else { b - knots[n - 1] };
        for (c, d1c) in [d1.x, d1.y, d1.z].into_iter().enumerate() {
            // sign (P[second] − P[first]) = (h1 / p) d1, or s · d1 with s free.
            let mut terms = vec![(3 * second + c, sign), (3 * first + c, -sign)];
            if free_scales[which] {
                terms.push((scale_index(which), -d1c));
                constraints.push(terms, 0.0);
            } else {
                constraints.push(terms, h1 / p as f64 * d1c);
            }
        }
        if let (Some(d2), true, true) = (ders[1], derivative_magnitudes, p >= 2 && n >= 3) {
            // C''(a) = p (p − 1) / h1 · ((P2 − P1) / h2 − (P1 − P0) / h1), mirrored at the end.
            let third = if which == 0 { 2 } else { n - 3 };
            let h2 = if which == 0 { knots[p + 2] - a } else { b - knots[n - 2] };
            let f = (p * (p - 1)) as f64 / h1;
            for (c, d2c) in [d2.x, d2.y, d2.z].into_iter().enumerate() {
                constraints.push(
                    vec![(3 * third + c, f / h2), (3 * second + c, -f / h2 - f / h1), (3 * first + c, f / h1)],
                    d2c,
                );
            }
        }
    }
    // The KKT system: [N Cᵀ; C 0] [x; λ] = [r; d].
    let size = unknowns + constraints.rows.len();
    let mut kkt = vec![vec![0.0; size]; size];
    let mut values = vec![0.0; size];
    for i in 0..unknowns {
        kkt[i][..unknowns].copy_from_slice(&normal[i]);
        values[i] = rhs[i];
    }
    for (r, (terms, value)) in constraints.rows.iter().enumerate() {
        for &(j, coefficient) in terms {
            kkt[unknowns + r][j] += coefficient;
            kkt[j][unknowns + r] += coefficient;
        }
        values[unknowns + r] = *value;
    }
    let x = solve(kkt, values)?;
    Some((0..n).map(|i| Vec3::new(x[3 * i], x[3 * i + 1], x[3 * i + 2])).collect())
}

/// Chord-length parameters for the targets' points, averaged over the targets, from 0 to 1.
fn chord_parameters(targets: &[Target]) -> Vec<f64> {
    let m = targets[0].points.len();
    let mut t = vec![0.0; m];
    for target in targets {
        let mut acc = vec![0.0; m];
        for k in 1..m {
            acc[k] = acc[k - 1] + norm(sub(target.points[k], target.points[k - 1]));
        }
        let total = acc[m - 1];
        for k in 0..m {
            t[k] += if total > 0.0 { acc[k] / total } else { k as f64 / (m - 1) as f64 };
        }
    }
    t.iter().map(|x| x / targets.len() as f64).collect()
}

fn clamped_knots(p: usize, a: f64, b: f64, interior: &[f64]) -> Vec<f64> {
    let mut knots = vec![a; p + 1];
    knots.extend_from_slice(interior);
    knots.extend(std::iter::repeat_n(b, p + 1));
    knots
}

/// Approximates the targets with consistently parameterized B-splines of degree `p` that keep within
/// `tolerance` of every point, adding knots where the fit is worst until it does or there are `max_poles` poles.
fn approximate(
    targets: &[Target],
    p: usize,
    tolerance: f64,
    given: Option<Vec<f64>>,
    max_poles: usize,
    interpolate: &[usize],
) -> R<Vec<Spline>> {
    let m = targets[0].points.len();
    if m < 2 || targets.iter().any(|t| t.points.len() != m) {
        return Err(error("@approximateSpline: every target needs the same number of positions, at least 2"));
    }
    let derivative_magnitudes = given.is_some();
    let ends = targets[0].start.iter().chain(&targets[0].end).filter(|d| d.is_some()).count();
    let available = m + targets[0].start[0].is_some() as usize + targets[0].end[0].is_some() as usize;
    let p = p.clamp(1, available - 1);
    let mut params = given.unwrap_or_else(|| chord_parameters(targets));
    let (a, b) = (params[0], params[m - 1]);
    let min_poles = (p + 1).max(2 + ends);
    let mut interior: Vec<f64> =
        (1..=(min_poles - p - 1)).map(|i| a + (b - a) * i as f64 / (min_poles - p) as f64).collect();
    let correct = !derivative_magnitudes && targets.len() == 1;
    loop {
        let knots = clamped_knots(p, a, b, &interior);
        let mut splines = Vec::new();
        for target in targets {
            let poles = fit(p, &knots, &params, target, interpolate, derivative_magnitudes)
                .ok_or_else(|| error("@approximateSpline: the fit failed"))?;
            splines.push(Spline { degree: p, knots: knots.clone(), poles });
        }
        // Moving each point's parameter to its closest point on the curve, then refitting, tightens the fit.
        if correct {
            for _ in 0..3 {
                let s = &splines[0];
                for (u, point) in params.iter_mut().zip(&targets[0].points).take(m - 1).skip(1) {
                    *u = s.project(*point, *u);
                }
                let poles = fit(p, &knots, &params, &targets[0], interpolate, false)
                    .ok_or_else(|| error("@approximateSpline: the fit failed"))?;
                splines[0].poles = poles;
            }
        }
        let errors: Vec<f64> = (0..m)
            .map(|k| {
                splines
                    .iter()
                    .zip(targets)
                    .map(|(s, t)| {
                        let u = if correct { s.project(t.points[k], params[k]) } else { params[k] };
                        norm(sub(s.eval(u)[0], t.points[k]))
                    })
                    .fold(0.0, f64::max)
            })
            .collect();
        let poles = interior.len() + p + 1;
        if errors.iter().all(|&e| e <= tolerance) || poles >= max_poles {
            return Ok(splines);
        }
        // Split each span holding a point that's too far off, at the middle of the points in it.
        let mut spans: Vec<(f64, f64)> = Vec::new();
        let mut bounds = vec![a];
        bounds.extend_from_slice(&interior);
        bounds.push(b);
        for w in bounds.windows(2) {
            let inside: Vec<usize> = (0..m).filter(|&k| params[k] >= w[0] && params[k] <= w[1]).collect();
            let worst = inside.iter().map(|&k| errors[k]).fold(0.0, f64::max);
            if worst > tolerance && inside.len() >= 2 {
                let (lo, hi) = (params[inside[0]], params[*inside.last().unwrap()]);
                let split = (lo + hi) / 2.0;
                if split > w[0] + 1e-12 && split < w[1] - 1e-12 {
                    spans.push((worst, split));
                }
            }
        }
        if spans.is_empty() {
            return Ok(splines);
        }
        spans.sort_by(|x, y| y.0.total_cmp(&x.0));
        spans.truncate(max_poles - poles);
        interior.extend(spans.iter().map(|s| s.1));
        interior.sort_by(f64::total_cmp);
    }
}

fn opt_vec3(v: &Value, what: &str) -> R<Option<Vec3>> {
    if v.is_undefined() { Ok(None) } else { values::vec3(v, what).map(Some) }
}

fn approximate_spline(tags: &Tags, definition: &Value) -> R<Value> {
    let number = |name: &str| field(definition, name).as_number();
    let degree = number("degree").ok_or_else(|| error("@approximateSpline: degree must be a number"))? as usize;
    let tolerance = values::length(field(definition, "tolerance"), "approximateSpline tolerance")?.max(1e-8);
    if values::opt_bool(definition, "isPeriodic")?.unwrap_or(false) {
        return Err(error("@approximateSpline: periodic splines aren't supported yet"));
    }
    let targets = field(definition, "targets")
        .as_array()
        .ok_or_else(|| error("@approximateSpline: targets must be an array"))?
        .iter()
        .map(|t| {
            let points = field(t, "positions")
                .as_array()
                .ok_or_else(|| error("@approximateSpline: a target needs positions"))?
                .iter()
                .map(|p| values::vec3(p, "approximateSpline position"))
                .collect::<R<Vec<_>>>()?;
            Ok(Target {
                points,
                start: [
                    opt_vec3(field(t, "startDerivative"), "startDerivative")?,
                    opt_vec3(field(t, "start2ndDerivative"), "start2ndDerivative")?,
                ],
                end: [
                    opt_vec3(field(t, "endDerivative"), "endDerivative")?,
                    opt_vec3(field(t, "end2ndDerivative"), "end2ndDerivative")?,
                ],
            })
        })
        .collect::<R<Vec<_>>>()?;
    if targets.is_empty() {
        return Err(error("@approximateSpline: no targets"));
    }
    let given = match field(definition, "parameters") {
        v if v.is_undefined() => None,
        v => Some(values::numbers(v, "approximateSpline parameters")?),
    };
    let max_poles = number("maxControlPoints").map_or(10000, |n| n as usize).max(2);
    let interpolate = match field(definition, "interpolateIndices") {
        v if v.is_undefined() => Vec::new(),
        v => values::numbers(v, "interpolateIndices")?.into_iter().map(|x| x as usize).collect(),
    };
    let splines = approximate(&targets, degree, tolerance, given, max_poles, &interpolate)?;
    let out = splines
        .iter()
        .map(|s| {
            let data = BSplineData {
                degree: s.degree as i32,
                periodic: false,
                rational: false,
                poles: s.poles.clone(),
                weights: Vec::new(),
                knots: s.knots.clone(),
            };
            bspline_value(tags, &data)
        })
        .collect::<R<Vec<_>>>()?;
    Ok(Value::array(out))
}

// ----- surface fitting -----

/// The eigenvectors of a symmetric 3×3 matrix, by Jacobi rotations, largest eigenvalue first.
fn eigenvectors(mut a: [[f64; 3]; 3]) -> [Vec3; 3] {
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..50 {
        let (mut p, mut q, mut largest) = (0, 1, 0.0);
        for (i, j) in [(0, 1), (0, 2), (1, 2)] {
            if a[i][j].abs() > largest {
                (p, q, largest) = (i, j, a[i][j].abs());
            }
        }
        if largest < 1e-300 {
            break;
        }
        let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
        let t = theta.signum().max(0.0).mul_add(2.0, -1.0) / (theta.abs() + (theta * theta + 1.0).sqrt());
        let (c, s) = (1.0 / (t * t + 1.0).sqrt(), t / (t * t + 1.0).sqrt());
        for row in a.iter_mut() {
            let (akp, akq) = (row[p], row[q]);
            row[p] = c * akp - s * akq;
            row[q] = s * akp + c * akq;
        }
        let (row_p, row_q) = (a[p], a[q]);
        for k in 0..3 {
            a[p][k] = c * row_p[k] - s * row_q[k];
            a[q][k] = s * row_p[k] + c * row_q[k];
        }
        for row in v.iter_mut() {
            let (vp, vq) = (row[p], row[q]);
            row[p] = c * vp - s * vq;
            row[q] = s * vp + c * vq;
        }
    }
    let mut order = [0usize, 1, 2];
    order.sort_by(|&i, &j| a[j][j].total_cmp(&a[i][i]));
    order.map(|k| Vec3::new(v[0][k], v[1][k], v[2][k]))
}

/// A bicubic B-spline surface: poles row by row, with its knot vectors.
pub struct SurfaceFit {
    pub poles: Vec<Vec3>,
    pub counts: (usize, usize),
    pub knots_u: Vec<f64>,
    pub knots_v: Vec<f64>,
}

/// A smooth bicubic surface within `tolerance` of the points, over the rectangle they span on their best-fit
/// plane: a height field over that plane, fitted by least squares with a little bending energy (more if
/// `smooth`), on finer control grids until it's close enough.
pub fn fit_surface(points: &[Vec3], tolerance: f64, smooth: bool) -> R<SurfaceFit> {
    if points.len() < 3 {
        return Err(regen_error("CONSTRAINED_SURFACE_TOO_FEW_POINTS"));
    }
    let m = points.len() as f64;
    let centre = points.iter().fold(Vec3::new(0.0, 0.0, 0.0), |acc, p| axpy(acc, 1.0 / m, *p));
    let mut covariance = [[0.0; 3]; 3];
    for p in points {
        let d = sub(*p, centre);
        let d = [d.x, d.y, d.z];
        for i in 0..3 {
            for j in 0..3 {
                covariance[i][j] += d[i] * d[j];
            }
        }
    }
    let [ax_u, ax_v, _] = eigenvectors(covariance);
    let normal =
        values::normalize(values::cross(ax_u, ax_v)).ok_or_else(|| regen_error("CONSTRAINED_SURFACE_FAILED"))?;
    let local: Vec<(f64, f64, f64)> = points
        .iter()
        .map(|p| {
            let d = sub(*p, centre);
            (values::dot(d, ax_u), values::dot(d, ax_v), values::dot(d, normal))
        })
        .collect();
    let range = |f: fn(&(f64, f64, f64)) -> f64| {
        local.iter().map(f).fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)))
    };
    let ((u0, u1), (v0, v1)) = (range(|p| p.0), range(|p| p.1));
    if u1 - u0 < 1e-12 || v1 - v0 < 1e-12 {
        return Err(regen_error("CONSTRAINED_SURFACE_FAILED"));
    }
    let params: Vec<(f64, f64)> = local.iter().map(|p| ((p.0 - u0) / (u1 - u0), (p.1 - v0) / (v1 - v0))).collect();
    let p = 3;
    let bending = if smooth { 1e-5 } else { 1e-7 };
    let mut n = p + 1;
    loop {
        let interior: Vec<f64> = (1..n - p).map(|i| i as f64 / (n - p) as f64).collect();
        let knots = clamped_knots(p, 0.0, 1.0, &interior);
        let unknowns = n * n;
        let mut normal_eq = vec![vec![0.0; unknowns]; unknowns];
        let mut rhs = vec![0.0; unknowns];
        let mut rows = Vec::with_capacity(params.len());
        for (k, &(u, v)) in params.iter().enumerate() {
            let (su, sv) = (find_span(n, p, u, &knots), find_span(n, p, v, &knots));
            let (bu, bv) = (basis_derivatives(su, u, p, 0, &knots), basis_derivatives(sv, v, p, 0, &knots));
            let mut row = Vec::with_capacity((p + 1) * (p + 1));
            for (a, nu) in bu[0].iter().enumerate() {
                for (b, nv) in bv[0].iter().enumerate() {
                    row.push(((su - p + a) * n + (sv - p + b), nu * nv));
                }
            }
            for &(i, wi) in &row {
                for &(j, wj) in &row {
                    normal_eq[i][j] += wi * wj;
                }
                rhs[i] += wi * local[k].2;
            }
            rows.push(row);
        }
        // Bending energy: squared second differences of the control net in each direction, and the twist.
        let weight = bending * params.len() as f64;
        let mut add_term = |terms: &[(usize, f64)]| {
            for &(i, wi) in terms {
                for &(j, wj) in terms {
                    normal_eq[i][j] += weight * wi * wj;
                }
            }
        };
        for i in 0..n {
            for j in 0..n {
                if i + 2 < n {
                    add_term(&[(i * n + j, 1.0), ((i + 1) * n + j, -2.0), ((i + 2) * n + j, 1.0)]);
                }
                if j + 2 < n {
                    add_term(&[(i * n + j, 1.0), (i * n + j + 1, -2.0), (i * n + j + 2, 1.0)]);
                }
                if i + 1 < n && j + 1 < n {
                    let r = std::f64::consts::SQRT_2;
                    add_term(&[(i * n + j, r), ((i + 1) * n + j, -r), (i * n + j + 1, -r), ((i + 1) * n + j + 1, r)]);
                }
            }
        }
        let heights = solve(normal_eq, rhs).ok_or_else(|| regen_error("CONSTRAINED_SURFACE_FAILED"))?;
        let worst = rows
            .iter()
            .zip(&local)
            .map(|(row, l)| (row.iter().map(|&(i, w)| w * heights[i]).sum::<f64>() - l.2).abs())
            .fold(0.0, f64::max);
        if worst <= tolerance || n >= 24 {
            // Poles over the Greville points, where a clamped spline's parameter maps linearly to the plane.
            let greville: Vec<f64> = (0..n).map(|i| knots[i + 1..=i + p].iter().sum::<f64>() / p as f64).collect();
            let mut poles = Vec::with_capacity(unknowns);
            for (i, gu) in greville.iter().enumerate() {
                for (j, gv) in greville.iter().enumerate() {
                    let q = axpy(centre, u0 + gu * (u1 - u0), ax_u);
                    let q = axpy(q, v0 + gv * (v1 - v0), ax_v);
                    poles.push(axpy(q, heights[i * n + j], normal));
                }
            }
            return Ok(SurfaceFit { poles, counts: (n, n), knots_u: knots.clone(), knots_v: knots });
        }
        n += 1;
    }
}

// ----- path deviation -----

/// The edges a query finds, with the edges of the bodies it finds.
fn path_edges(model: &Model, q: &Value) -> R<Vec<Shape>> {
    let mut out: Vec<Shape> = Vec::new();
    for r in query::evaluate(model, q)? {
        let refs = match r.kind {
            EntityType::Edge => vec![r],
            EntityType::Body => model.owned_by(r.body, Some(EntityType::Edge)),
            _ => Vec::new(),
        };
        for e in refs {
            let shape = model.shape(e);
            if !out.iter().any(|s| s.is_same(shape)) {
                out.push(shape.clone());
            }
        }
    }
    Ok(out)
}

/// The farthest any sample of `from` gets from `to`: the distance, the sample and its closest point on `to`.
fn farthest(from: &[Shape], to: &Shape) -> R<(f64, Vec3, Vec3)> {
    const SAMPLES: usize = 200;
    let mut best = (0.0, Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0));
    for edge in from {
        for i in 0..=SAMPLES {
            let p = edge.edge_tangent(i as f64 / SAMPLES as f64, true).map_err(occt_error)?.origin;
            let d = occt::vertex(p).and_then(|v| v.distance(to)).map_err(occt_error)?;
            if d.distance > best.0 {
                best = (d.distance, p, d.b);
            }
        }
    }
    Ok(best)
}

fn max_path_deviation(tags: &Tags, model: &Model, definition: &Value) -> R<Value> {
    let side1 = path_edges(model, field(definition, "side1"))?;
    let side2 = path_edges(model, field(definition, "side2"))?;
    if side1.is_empty() || side2.is_empty() {
        return Err(regen_error("CANNOT_RESOLVE_ENTITIES"));
    }
    let (one, two) = (occt::compound(&side1), occt::compound(&side2));
    let a = farthest(&side1, &two)?;
    let b = farthest(&side2, &one)?;
    let (deviation, p1, p2) = if a.0 >= b.0 { a } else { (b.0, b.2, b.1) };
    Ok(map(vec![
        ("deviation", tags.length(deviation)),
        ("side1Point", tags.length_vector(p1)),
        ("side2Point", tags.length_vector(p2)),
    ]))
}

pub fn register(m: &mut GeomBuiltins) {
    m.insert("evApproximateBSplineCurve", |interp, args| {
        let tags = tags(interp)?;
        let definition = arg(&args, 1, "evApproximateBSplineCurve")?.clone();
        let tolerance = field(&definition, "tolerance").as_number().unwrap_or(1e-6);
        let force_cubic = values::opt_bool(&definition, "forceCubic")?.unwrap_or(false);
        let force_non_rational = values::opt_bool(&definition, "forceNonRational")?.unwrap_or(false);
        model_mut(&args, "evApproximateBSplineCurve", |model| {
            let found = query::evaluate(model, field(&definition, "edge"))?;
            let r = found.first().ok_or_else(|| regen_error("CANNOT_RESOLVE_ENTITIES"))?;
            if r.kind != EntityType::Edge {
                return Err(regen_error("INVALID_INPUT"));
            }
            let data = model.shape(*r).edge_bspline(tolerance, force_cubic, force_non_rational).map_err(occt_error)?;
            bspline_value(&tags, &data)
        })
    });
    m.insert("approximateSpline", |interp, args| {
        let tags = tags(interp)?;
        approximate_spline(&tags, arg(&args, 1, "approximateSpline")?)
    });
    m.insert("evMaxPathDeviation", |interp, args| {
        let tags = tags(interp)?;
        let definition = arg(&args, 1, "evMaxPathDeviation")?.clone();
        model_mut(&args, "evMaxPathDeviation", |model| max_path_deviation(&tags, model, &definition))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basis_functions_sum_to_one_and_derivatives_to_zero() {
        let knots = [0.0, 0.0, 0.0, 0.0, 0.3, 0.7, 1.0, 1.0, 1.0, 1.0];
        for &u in &[0.0, 0.1, 0.3, 0.5, 0.99, 1.0] {
            let span = find_span(6, 3, u, &knots);
            let d = basis_derivatives(span, u, 3, 2, &knots);
            assert!((d[0].iter().sum::<f64>() - 1.0).abs() < 1e-12);
            assert!(d[1].iter().sum::<f64>().abs() < 1e-9);
            assert!(d[2].iter().sum::<f64>().abs() < 1e-8);
        }
    }

    #[test]
    fn approximates_a_quarter_circle_within_tolerance() {
        let points: Vec<Vec3> = (0..=100)
            .map(|i| {
                let t = std::f64::consts::FRAC_PI_2 * i as f64 / 100.0;
                Vec3::new(t.cos(), t.sin(), 0.0)
            })
            .collect();
        let target = Target { points: points.clone(), start: [None, None], end: [None, None] };
        let splines = approximate(&[target], 3, 1e-5, None, 50, &[]).unwrap();
        let s = &splines[0];
        assert!(s.poles.len() < 20, "{} poles", s.poles.len());
        for k in 0..=200 {
            let u = k as f64 / 200.0;
            let p = s.eval(u)[0];
            assert!((norm(p) - 1.0).abs() < 2e-5, "{u}: {}", norm(p));
        }
        assert!(norm(sub(s.poles[0], points[0])) < 1e-12);
    }

    #[test]
    fn keeps_end_directions() {
        let points: Vec<Vec3> = (0..=50).map(|i| Vec3::new(i as f64 / 50.0, (i as f64 / 50.0).powi(2), 0.0)).collect();
        let target = Target {
            points,
            start: [Some(Vec3::new(1.0, 0.0, 0.0)), None],
            end: [Some(Vec3::new(1.0, 2.0, 0.0)), None],
        };
        let s = &approximate(&[target], 3, 1e-6, None, 30, &[]).unwrap()[0];
        let d0 = s.eval(0.0)[1];
        assert!(d0.y.abs() < 1e-9 * d0.x.abs().max(1.0), "{d0:?}");
        let d1 = s.eval(1.0)[1];
        assert!((d1.y - 2.0 * d1.x).abs() < 1e-8 * d1.x.abs().max(1.0), "{d1:?}");
    }

    #[test]
    fn eigenvectors_of_a_stretched_cloud() {
        let [a, b, c] = eigenvectors([[4.0, 1.0, 0.0], [1.0, 3.0, 0.0], [0.0, 0.0, 0.5]]);
        assert!(c.z.abs() > 1.0 - 1e-12, "{c:?}");
        assert!(values::dot(a, b).abs() < 1e-12);
        // The largest eigenvalue of [[4, 1], [1, 3]] is (7 + √5) / 2, along (1, (√5 − 1) / 2).
        let expected = values::normalize(Vec3::new(1.0, (5f64.sqrt() - 1.0) / 2.0, 0.0)).unwrap();
        assert!(values::dot(a, expected).abs() > 1.0 - 1e-12, "{a:?}");
    }

    #[test]
    fn surface_passes_near_its_points() {
        let points: Vec<Vec3> =
            [(0., 0., 0.), (20., 0., 0.), (40., 0., 0.), (0., 15., 0.), (20., 15., 4.), (40., 15., 0.)]
                .iter()
                .chain(&[(0., 30., 0.), (20., 30., 0.), (40., 30., 0.)])
                .map(|&(x, y, z)| Vec3::new(x * 1e-3, y * 1e-3, z * 1e-3))
                .collect();
        let fit = fit_surface(&points, 1e-5, true).unwrap();
        let face = occt::bspline_surface_face(&fit.poles, fit.counts, &fit.knots_u, &fit.knots_v, (3, 3)).unwrap();
        for p in &points {
            let d = occt::vertex(*p).unwrap().distance(&face).unwrap().distance;
            assert!(d < 1.1e-5, "{p:?} is {d} from the surface");
        }
    }
}
