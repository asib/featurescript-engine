//! Matrices are arrays of equal-length row arrays of numbers.

use super::{BuiltinFn, arg, index};
use crate::eval::{R, error};
use crate::value::{Kind, Value};
use std::collections::HashMap;

type M = Vec<Vec<f64>>;

fn to_matrix(v: &Value, name: &str) -> R<M> {
    let bad = || error(format!("@{name}: expected a matrix, not {}", v.type_description()));
    let rows = v.as_array().ok_or_else(bad)?;
    let mut m = Vec::with_capacity(rows.len());
    for row in rows.iter() {
        let row = row.as_array().ok_or_else(bad)?;
        m.push(row.iter().map(|x| x.as_number().ok_or_else(bad)).collect::<R<Vec<f64>>>()?);
    }
    if m.iter().any(|r| r.len() != m[0].len()) {
        return Err(bad());
    }
    Ok(m)
}

fn from_matrix(m: M) -> Value {
    Value::array(m.into_iter().map(|row| Value::array(row.into_iter().map(Value::number).collect())).collect())
}

fn elementwise(name: &str, a: &Value, b: &Value, f: fn(f64, f64) -> f64) -> R<Value> {
    let (a, b) = (to_matrix(a, name)?, to_matrix(b, name)?);
    if a.len() != b.len() || a.first().map(Vec::len) != b.first().map(Vec::len) {
        return Err(error(format!("@{name}: matrix sizes differ")));
    }
    Ok(from_matrix(a.iter().zip(&b).map(|(ra, rb)| ra.iter().zip(rb).map(|(x, y)| f(*x, *y)).collect()).collect()))
}

fn multiply(a: &M, b: &M) -> R<M> {
    let inner = a.first().map_or(0, Vec::len);
    if inner != b.len() {
        return Err(error("@matrixMultiply: sizes don't match"));
    }
    let cols = b.first().map_or(0, Vec::len);
    Ok(a.iter().map(|row| (0..cols).map(|j| (0..inner).map(|k| row[k] * b[k][j]).sum()).collect()).collect())
}

#[allow(clippy::needless_range_loop)]
fn determinant(m: &M) -> f64 {
    let n = m.len();
    let mut a = m.clone();
    let mut det = 1.0;
    for col in 0..n {
        let pivot = (col..n).max_by(|&i, &j| a[i][col].abs().partial_cmp(&a[j][col].abs()).unwrap()).unwrap();
        if a[pivot][col] == 0.0 {
            return 0.0;
        }
        if pivot != col {
            a.swap(pivot, col);
            det = -det;
        }
        det *= a[col][col];
        for row in col + 1..n {
            let factor = a[row][col] / a[col][col];
            for k in col..n {
                a[row][k] -= factor * a[col][k];
            }
        }
    }
    det
}

#[allow(clippy::needless_range_loop)]
fn inverse(m: &M) -> R<M> {
    let n = m.len();
    let mut a: M = m
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut r = row.clone();
            r.extend((0..n).map(|j| if i == j { 1.0 } else { 0.0 }));
            r
        })
        .collect();
    for col in 0..n {
        let pivot = (col..n).max_by(|&i, &j| a[i][col].abs().partial_cmp(&a[j][col].abs()).unwrap()).unwrap();
        if a[pivot][col].abs() < 1e-300 {
            return Err(error("@matrixInverse: matrix is singular"));
        }
        a.swap(pivot, col);
        let p = a[col][col];
        for k in 0..2 * n {
            a[col][k] /= p;
        }
        for row in 0..n {
            if row != col {
                let factor = a[row][col];
                for k in 0..2 * n {
                    a[row][k] -= factor * a[col][k];
                }
            }
        }
    }
    Ok(a.into_iter().map(|row| row[n..].to_vec()).collect())
}

/// A singular value decomposition `m = u s vᵀ` by one-sided Jacobi rotations: `u` n × n and `v` p × p orthogonal, `s`
/// n × p with the singular values down its diagonal, largest first.
#[allow(clippy::needless_range_loop)]
fn svd(m: &M) -> (M, M, M) {
    let (n, p) = (m.len(), m.first().map_or(0, Vec::len));
    let mut w = m.clone();
    let mut v: M = (0..p).map(|i| (0..p).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
    for _ in 0..100 {
        let mut rotated = false;
        for i in 0..p {
            for j in i + 1..p {
                let (mut a, mut b, mut g) = (0.0, 0.0, 0.0);
                for k in 0..n {
                    a += w[k][i] * w[k][i];
                    b += w[k][j] * w[k][j];
                    g += w[k][i] * w[k][j];
                }
                if g.abs() <= 1e-15 * (a * b).sqrt() || g == 0.0 {
                    continue;
                }
                rotated = true;
                let zeta = (b - a) / (2.0 * g);
                let t = zeta.signum() / (zeta.abs() + (1.0 + zeta * zeta).sqrt());
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = c * t;
                for row in w.iter_mut().chain(v.iter_mut()) {
                    let (x, y) = (row[i], row[j]);
                    row[i] = c * x - s * y;
                    row[j] = s * x + c * y;
                }
            }
        }
        if !rotated {
            break;
        }
    }
    let norms: Vec<f64> = (0..p).map(|j| (0..n).map(|k| w[k][j] * w[k][j]).sum::<f64>().sqrt()).collect();
    let mut order: Vec<usize> = (0..p).collect();
    order.sort_by(|&a, &b| norms[b].partial_cmp(&norms[a]).unwrap_or(std::cmp::Ordering::Equal));
    let largest = norms.iter().cloned().fold(0.0, f64::max);
    let mut u: M = vec![vec![0.0; n]; n];
    let mut s: M = vec![vec![0.0; p]; n];
    let mut columns: Vec<Vec<f64>> = Vec::new();
    for (j, &c) in order.iter().enumerate().take(n.min(p)) {
        s[j][j] = norms[c];
        if norms[c] > 1e-14 * largest.max(1e-300) {
            columns.push((0..n).map(|k| w[k][c] / norms[c]).collect());
        } else {
            break;
        }
    }
    // The rest of u: the standard basis made orthogonal to what's there.
    for e in 0..n {
        if columns.len() == n {
            break;
        }
        let mut x: Vec<f64> = (0..n).map(|k| if k == e { 1.0 } else { 0.0 }).collect();
        for c in &columns {
            let d: f64 = (0..n).map(|k| x[k] * c[k]).sum();
            for k in 0..n {
                x[k] -= d * c[k];
            }
        }
        let norm = x.iter().map(|a| a * a).sum::<f64>().sqrt();
        if norm > 1e-8 {
            columns.push(x.into_iter().map(|a| a / norm).collect());
        }
    }
    for (j, c) in columns.iter().enumerate() {
        for k in 0..n {
            u[k][j] = c[k];
        }
    }
    let v = (0..p).map(|k| order.iter().map(|&c| v[k][c]).collect()).collect();
    (u, s, v)
}

pub fn register(m: &mut HashMap<&'static str, BuiltinFn>) {
    m.insert("isMatrix", |_, args| {
        Ok(Value::bool(to_matrix(arg(&args, 0, "isMatrix")?, "isMatrix").is_ok_and(|m| !m.is_empty())))
    });
    m.insert("matrixIdentity", |_, args| {
        let n = index(&args, 0, "matrixIdentity")?;
        Ok(from_matrix((0..n).map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect()))
    });
    m.insert("matrixSum", |_, args| {
        elementwise("matrixSum", arg(&args, 0, "matrixSum")?, arg(&args, 1, "matrixSum")?, |x, y| x + y)
    });
    m.insert("matrixDifference", |_, args| {
        elementwise(
            "matrixDifference",
            arg(&args, 0, "matrixDifference")?,
            arg(&args, 1, "matrixDifference")?,
            |x, y| x - y,
        )
    });
    m.insert("matrixCwiseProduct", |_, args| {
        elementwise(
            "matrixCwiseProduct",
            arg(&args, 0, "matrixCwiseProduct")?,
            arg(&args, 1, "matrixCwiseProduct")?,
            |x, y| x * y,
        )
    });
    m.insert("matrixNegate", |_, args| {
        let a = to_matrix(arg(&args, 0, "matrixNegate")?, "matrixNegate")?;
        Ok(from_matrix(a.into_iter().map(|r| r.into_iter().map(|x| -x).collect()).collect()))
    });
    // Matrix times matrix, or a matrix scaled by a number on either side.
    m.insert("matrixMultiply", |_, args| {
        let (a, b) = (arg(&args, 0, "matrixMultiply")?, arg(&args, 1, "matrixMultiply")?);
        let scale = |m: &Value, s: f64| -> R<Value> {
            let m = to_matrix(m, "matrixMultiply")?;
            Ok(from_matrix(m.into_iter().map(|r| r.into_iter().map(|x| x * s).collect()).collect()))
        };
        // A vector (an array of numbers) on the right is a column; the result is a vector again.
        let as_vector = |v: &Value| -> Option<Vec<f64>> { v.as_array()?.iter().map(Value::as_number).collect() };
        match (&a.kind, &b.kind) {
            (_, Kind::Number(s)) => scale(a, *s),
            (Kind::Number(s), _) => scale(b, *s),
            _ => {
                let left = to_matrix(a, "matrixMultiply")?;
                if let Some(column) = as_vector(b) {
                    let product = multiply(&left, &column.into_iter().map(|x| vec![x]).collect())?;
                    return Ok(Value::array(product.into_iter().map(|row| Value::number(row[0])).collect()));
                }
                Ok(from_matrix(multiply(&left, &to_matrix(b, "matrixMultiply")?)?))
            }
        }
    });
    // Rotation by `angle` radians about `axis` (normalised here), by Rodrigues' formula.
    m.insert("matrixRotation3d", |_, args| {
        let axis = arg(&args, 0, "matrixRotation3d")?;
        let angle = super::number(&args, 1, "matrixRotation3d")?;
        let v: Vec<f64> = axis
            .as_array()
            .and_then(|a| a.iter().map(Value::as_number).collect::<Option<Vec<f64>>>())
            .filter(|v| v.len() == 3)
            .ok_or_else(|| error("@matrixRotation3d: axis must be a 3D vector"))?;
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if n == 0.0 {
            return Err(error("@matrixRotation3d: zero axis"));
        }
        let (x, y, z) = (v[0] / n, v[1] / n, v[2] / n);
        let (s, c) = angle.sin_cos();
        let t = 1.0 - c;
        Ok(from_matrix(vec![
            vec![t * x * x + c, t * x * y - s * z, t * x * z + s * y],
            vec![t * x * y + s * z, t * y * y + c, t * y * z - s * x],
            vec![t * x * z - s * y, t * y * z + s * x, t * z * z + c],
        ]))
    });
    m.insert("matrixTranspose", |_, args| {
        let a = to_matrix(arg(&args, 0, "matrixTranspose")?, "matrixTranspose")?;
        let cols = a.first().map_or(0, Vec::len);
        Ok(from_matrix((0..cols).map(|j| a.iter().map(|row| row[j]).collect()).collect()))
    });
    m.insert("matrixSquaredNorm", |_, args| {
        let a = to_matrix(arg(&args, 0, "matrixSquaredNorm")?, "matrixSquaredNorm")?;
        Ok(Value::number(a.iter().flatten().map(|x| x * x).sum()))
    });
    m.insert("matrixDeterminant", |_, args| {
        let a = to_matrix(arg(&args, 0, "matrixDeterminant")?, "matrixDeterminant")?;
        if a.len() != a.first().map_or(0, Vec::len) {
            return Err(error("@matrixDeterminant: matrix must be square"));
        }
        Ok(Value::number(determinant(&a)))
    });
    m.insert("matrixInverse", |_, args| {
        let a = to_matrix(arg(&args, 0, "matrixInverse")?, "matrixInverse")?;
        if a.len() != a.first().map_or(0, Vec::len) {
            return Err(error("@matrixInverse: matrix must be square"));
        }
        Ok(from_matrix(inverse(&a)?))
    });
    m.insert("matrixSvd", |_, args| {
        let a = to_matrix(arg(&args, 0, "matrixSvd")?, "matrixSvd")?;
        let (u, s, v) = svd(&a);
        Ok(super::map_value(vec![("u", from_matrix(u)), ("s", from_matrix(s)), ("v", from_matrix(v))]))
    });
}
