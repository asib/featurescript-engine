//! Feature patterns that reapply their features in each instance (`fullFeaturePattern`): the stack of instance
//! transforms std pushes around each instance's features, and the transforms features read from it.

use crate::model::{FeatureId, GeomState};
use crate::values::{self, field};
use crate::{GeomBuiltins, query, state_mut};
use fs_interp::Value;
use fs_interp::builtins::arg;
use fs_interp::eval::{R, error};
use occt::Transform;
use std::rc::Rc;

const IDENTITY: [f64; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];

/// `a` after `b`: the transform taking p to a(b(p)).
fn compose(a: &[f64; 12], b: &[f64; 12]) -> [f64; 12] {
    let mut m = [0.0; 12];
    for i in 0..3 {
        for j in 0..4 {
            let mut v = (0..3).map(|k| a[i * 4 + k] * b[k * 4 + j]).sum::<f64>();
            if j == 3 {
                v += a[i * 4 + 3];
            }
            m[i * 4 + j] = v;
        }
    }
    m
}

fn inverse(t: &[f64; 12]) -> R<[f64; 12]> {
    let a = |i: usize, j: usize| t[i * 4 + j];
    let det = a(0, 0) * (a(1, 1) * a(2, 2) - a(1, 2) * a(2, 1)) - a(0, 1) * (a(1, 0) * a(2, 2) - a(1, 2) * a(2, 0))
        + a(0, 2) * (a(1, 0) * a(2, 1) - a(1, 1) * a(2, 0));
    if det.abs() < 1e-15 {
        return Err(error("pattern transform can't be inverted"));
    }
    let cof = |i: usize, j: usize| {
        let (r0, r1) = ((i + 1) % 3, (i + 2) % 3);
        let (c0, c1) = ((j + 1) % 3, (j + 2) % 3);
        a(r0, c0) * a(r1, c1) - a(r0, c1) * a(r1, c0)
    };
    let mut m = [0.0; 12];
    for i in 0..3 {
        for j in 0..3 {
            // The inverse is the transposed cofactor matrix over the determinant.
            m[i * 4 + j] = cof(j, i) / det;
        }
    }
    for i in 0..3 {
        m[i * 4 + 3] = -(0..3).map(|k| m[i * 4 + k] * t[k * 4 + 3]).sum::<f64>();
    }
    Ok(m)
}

/// The transforms on the stack composed, outermost last applied.
fn full(state: &GeomState) -> [f64; 12] {
    state.pattern.iter().fold(IDENTITY, |acc, (_, t)| compose(&acc, &t.m))
}

/// A transform as builtins return it: plain numbers, which std's `transformFromBuiltin` gives metres.
fn to_value(m: &[f64; 12]) -> Value {
    let row = |i: usize| Value::array((0..3).map(|j| Value::number(m[i * 4 + j])).collect());
    values::map(vec![
        ("linear", Value::array((0..3).map(row).collect())),
        ("translation", Value::array((0..3).map(|i| Value::number(m[i * 4 + 3])).collect())),
    ])
}

fn starts_with(id: &FeatureId, prefix: &FeatureId) -> bool {
    id.len() >= prefix.len() && id.iter().zip(prefix.iter()).all(|(a, b)| a == b)
}

pub fn register(m: &mut GeomBuiltins) {
    m.insert("setFeaturePatternInstanceData", |_, args| {
        let id = values::feature_id(arg(&args, 1, "setFeaturePatternInstanceData")?, "pattern instance id")?;
        let transform = crate::ops::transform(field(arg(&args, 2, "setFeaturePatternInstanceData")?, "transform"))?;
        state_mut(&args, "setFeaturePatternInstanceData", |state| {
            Rc::make_mut(&mut state.pattern).push((id, transform));
            Ok(Value::UNDEFINED)
        })
    });
    m.insert("unsetFeaturePatternInstanceData", |_, args| {
        let id = values::feature_id(arg(&args, 1, "unsetFeaturePatternInstanceData")?, "pattern instance id")?;
        state_mut(&args, "unsetFeaturePatternInstanceData", |state| {
            if state.pattern.last().is_none_or(|(top, _)| *top != id) {
                return Err(error("@unsetFeaturePatternInstanceData: not the innermost pattern instance"));
            }
            Rc::make_mut(&mut state.pattern).pop();
            Ok(Value::UNDEFINED)
        })
    });
    m.insert("isInFeaturePattern", |_, args| {
        state_mut(&args, "isInFeaturePattern", |state| Ok(Value::bool(!state.pattern.is_empty())))
    });
    m.insert("getFullPatternTransform", |_, args| {
        state_mut(&args, "getFullPatternTransform", |state| Ok(to_value(&full(state))))
    });
    // The part of the pattern transform the references haven't had: with S the transforms down to the deepest
    // instance that made any of them, and F all of them, R with R S = F.
    m.insert("getRemainderPatternTransform", |_, args| {
        let definition = arg(&args, 1, "getRemainderPatternTransform")?.clone();
        state_mut(&args, "getRemainderPatternTransform", |state| {
            let f = full(state);
            if state.pattern.is_empty() {
                return Ok(to_value(&f));
            }
            let found = query::evaluate(&state.model, field(&definition, "references"))?;
            let deepest = found
                .iter()
                .filter_map(|r| {
                    let made_by = &state.model.info(*r).created_by;
                    state.pattern.iter().rposition(|(instance, _)| starts_with(made_by, instance))
                })
                .max();
            let s = match deepest {
                None => IDENTITY,
                Some(k) => state.pattern[..=k].iter().fold(IDENTITY, |acc, (_, t)| compose(&acc, &t.m)),
            };
            Ok(to_value(&compose(&f, &inverse(&s)?)))
        })
    });
    // The features' values in the order of their ids.
    m.insert("valuesSortedById", |_, args| {
        let features = arg(&args, 1, "valuesSortedById")?;
        let entries = features.as_map().ok_or_else(|| error("@valuesSortedById: expected a map"))?;
        Ok(Value::array(entries.values().cloned().collect()))
    });
    // Whether any of the features made a sketch.
    m.insert("containsSketch", |_, args| {
        let features = arg(&args, 1, "containsSketch")?.clone();
        let entries = features.as_map().ok_or_else(|| error("@containsSketch: expected a map"))?;
        let ids = entries.keys().map(|k| values::feature_id(k, "feature id")).collect::<R<Vec<_>>>()?;
        state_mut(&args, "containsSketch", |state| {
            Ok(Value::bool(state.model.bodies.iter().any(|b| {
                b.sketch_plane.is_some() && ids.iter().any(|id| starts_with(&b.info.created_by, id))
            })))
        })
    });
}

/// A transform std passes an operation to apply first (`transform` in a definition), if it isn't the identity.
pub fn transform_of(definition: &Value) -> R<Option<Transform>> {
    match field(definition, "transform") {
        v if v.is_undefined() => Ok(None),
        v => {
            let t = crate::ops::transform(v)?;
            Ok(if t.m.iter().zip(IDENTITY.iter()).all(|(a, b)| (a - b).abs() < 1e-15) { None } else { Some(t) })
        }
    }
}
