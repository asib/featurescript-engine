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

pub fn register(m: &mut GeomBuiltins) {
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
