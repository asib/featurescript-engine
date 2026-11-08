//! The builtins behind std's pattern features: the transforms of each instance, and the pattern data std records
//! for the UI.

use crate::values::{self, Tags, field, map};
use crate::{GeomBuiltins, tags};
use fs_interp::Value;
use fs_interp::builtins::arg;
use fs_interp::eval::{R, error};
use occt::Vec3;

const IDENTITY: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn integer(definition: &Value, key: &str, default: f64) -> R<i64> {
    match field(definition, key) {
        v if v.is_undefined() => Ok(default as i64),
        v => v.as_number().map(|n| n as i64).ok_or_else(|| error(format!("{key} must be a number"))),
    }
}

/// The instances to skip, as (index1, index2) pairs; `index2` is 0 for one-dimensional patterns.
fn skipped(definition: &Value) -> R<Vec<(i64, i64)>> {
    if !values::opt_bool(definition, "skipInstances")?.unwrap_or(false) {
        return Ok(Vec::new());
    }
    let Some(items) = field(definition, "skippedInstances").as_array() else { return Ok(Vec::new()) };
    let index = |item: &Value, key: &str| item.get(key).and_then(Value::as_number).map(|n| n as i64);
    Ok(items
        .iter()
        .map(|item| (index(item, "index1").or(index(item, "index")).unwrap_or(0), index(item, "index2").unwrap_or(0)))
        .collect())
}

fn result(transforms: Vec<Value>, names: Vec<String>) -> Value {
    map(vec![
        ("transforms", Value::array(transforms)),
        ("instanceNames", Value::array(names.into_iter().map(Value::string).collect())),
        ("manipulatorPoints", Value::array(Vec::new())),
    ])
}

fn linear_transforms(tags: &Tags, definition: &Value) -> R<Value> {
    let offset1 = values::vec3(field(definition, "offset1"), "offset1")?;
    let offset2 = values::vec3(field(definition, "offset2"), "offset2")?;
    let count1 = integer(definition, "instanceCount", 1.0)?;
    let count2 = integer(definition, "instanceCountTwo", 1.0)?;
    let centered = |key| values::opt_bool(definition, key).map(|b| b.unwrap_or(false));
    let start1 = if centered("isCentered")? { 1 - count1 } else { 0 };
    let start2 = if centered("isCenteredTwo")? { 1 - count2 } else { 0 };
    let skip = skipped(definition)?;
    let (mut transforms, mut names) = (Vec::new(), Vec::new());
    for j in start2..count2 {
        let suffix = if j == 0 { String::new() } else { format!("_{j}") };
        for i in start1..count1 {
            if (i == 0 && j == 0) || skip.contains(&(i, j)) {
                continue;
            }
            let t = values::add(values::scale(offset1, i as f64), values::scale(offset2, j as f64));
            transforms.push(tags.transform(IDENTITY, t));
            names.push(format!("{i}{suffix}"));
        }
    }
    Ok(result(transforms, names))
}

fn circular_transforms(tags: &Tags, definition: &Value) -> R<Value> {
    let axis = values::line(field(definition, "axis"), "axis")?;
    let angle = values::angle(field(definition, "angle"), "angle")?;
    let count = integer(definition, "instanceCount", 1.0)?;
    let start = if values::opt_bool(definition, "isCentered")?.unwrap_or(false) { 1 - count } else { 0 };
    let skip = skipped(definition)?;
    let (mut transforms, mut names) = (Vec::new(), Vec::new());
    for i in start..count {
        if i == 0 || skip.contains(&(i, 0)) {
            continue;
        }
        let m = crate::ops::rotation(axis, i as f64 * angle).m;
        let linear = [[m[0], m[1], m[2]], [m[4], m[5], m[6]], [m[8], m[9], m[10]]];
        transforms.push(tags.transform(linear, Vec3::new(m[3], m[7], m[11])));
        names.push(i.to_string());
    }
    Ok(result(transforms, names))
}

type M3 = [[f64; 3]; 3];

fn mul(a: &M3, b: &M3) -> M3 {
    let mut out = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

fn apply(a: &M3, v: Vec3) -> Vec3 {
    Vec3::new(
        a[0][0] * v.x + a[0][1] * v.y + a[0][2] * v.z,
        a[1][0] * v.x + a[1][1] * v.y + a[1][2] * v.z,
        a[2][0] * v.x + a[2][1] * v.y + a[2][2] * v.z,
    )
}

fn rotation_matrix(axis: Vec3, angle: f64) -> M3 {
    let m = crate::ops::rotation(occt::Axis { origin: Vec3::new(0.0, 0.0, 0.0), direction: axis }, angle).m;
    [[m[0], m[1], m[2]], [m[4], m[5], m[6]], [m[8], m[9], m[10]]]
}

/// std's `rotationMatrix3d(from, to)`: the smallest rotation taking one direction to another.
fn rotation_between(from: Vec3, to: Vec3) -> M3 {
    let axis = values::cross(from, to);
    if values::dot(axis, axis) < 1e-16 {
        if values::dot(from, to) > 0.0 {
            return IDENTITY;
        }
        let perp = values::canonical_x(values::normalize(from).unwrap_or(from));
        return rotation_matrix(perp, std::f64::consts::PI);
    }
    rotation_matrix(values::normalize(axis).expect("non-zero axis"), values::norm(axis).atan2(values::dot(from, to)))
}

/// A frame as (rotation whose columns are x, y, z; origin).
fn frame_matrix(f: &occt::Frame) -> (M3, Vec3) {
    let y = values::cross(f.normal, f.x);
    ([[f.x.x, y.x, f.normal.x], [f.x.y, y.y, f.normal.y], [f.x.z, y.z, f.normal.z]], f.origin)
}

/// The transform taking one tangent (a `Line` or a `Plane`) to another, as std's `transform(from, to)`.
fn between(from: &Value, to: &Value) -> R<(M3, Vec3)> {
    if from.get("direction").is_some() {
        let (a, b) = (values::line(from, "tangent")?, values::line(to, "tangent")?);
        let r = rotation_between(a.direction, b.direction);
        return Ok((r, values::sub(b.origin, apply(&r, a.origin))));
    }
    let (a, b) = (values::plane(from, "tangent")?, values::plane(to, "tangent")?);
    let (ra, oa) = frame_matrix(&a);
    let (rb, ob) = frame_matrix(&b);
    let ra_t = [[ra[0][0], ra[1][0], ra[2][0]], [ra[0][1], ra[1][1], ra[2][1]], [ra[0][2], ra[1][2], ra[2][2]]];
    let r = mul(&rb, &ra_t);
    Ok((r, values::sub(ob, apply(&r, oa))))
}

fn curve_transforms(tags: &Tags, definition: &Value) -> R<Value> {
    let tangents = field(definition, "tangents").as_array().ok_or_else(|| error("tangents must be an array"))?.clone();
    let count = integer(definition, "instanceCount", tangents.len() as f64)?.max(0) as usize;
    let skip = skipped(definition)?;
    // Instance i's transform: tangent i-1 to tangent i, after instance i-1's.
    let mut current: Option<(M3, Vec3)> = None;
    let (mut transforms, mut names) = (Vec::new(), Vec::new());
    for i in 1..count.min(tangents.len()) {
        let (r, t) = between(&tangents[i - 1], &tangents[i])?;
        current = Some(match current {
            None => (r, t),
            Some((pr, pt)) => (mul(&r, &pr), values::add(apply(&r, pt), t)),
        });
        if skip.contains(&(i as i64, 0)) {
            continue;
        }
        let (r, t) = current.expect("set above");
        transforms.push(tags.transform(r, t));
        names.push(i.to_string());
    }
    Ok(result(transforms, names))
}

pub fn register(m: &mut GeomBuiltins) {
    m.insert("computeCurvePatternTransforms", |interp, args| {
        let tags = tags(interp)?;
        curve_transforms(&tags, arg(&args, 1, "computeCurvePatternTransforms")?)
    });
    m.insert("computeLinearPatternTransforms", |interp, args| {
        let tags = tags(interp)?;
        linear_transforms(&tags, arg(&args, 1, "computeLinearPatternTransforms")?)
    });
    m.insert("computeCircularPatternTransforms", |interp, args| {
        let tags = tags(interp)?;
        circular_transforms(&tags, arg(&args, 1, "computeCircularPatternTransforms")?)
    });
    // Pattern data only drives the UI (instance manipulators and the like).
    m.insert("setPatternData", |_, _| Ok(Value::UNDEFINED));
}
