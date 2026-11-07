use super::{BuiltinFn, array, checked, number};
use crate::eval::{Interp, R, error};
use crate::value::Value;
use std::collections::HashMap;

macro_rules! unary {
    ($map:ident, $($name:literal => $f:expr),* $(,)?) => {
        $(
            $map.insert($name, |_: &mut Interp, args: Vec<Value>| -> R<Value> {
                let x = number(&args, 0, $name)?;
                let f: fn(f64) -> f64 = $f;
                checked($name, f(x))
            });
        )*
    };
}

pub fn register(map: &mut HashMap<&'static str, BuiltinFn>) {
    unary!(map,
        "sqrt" => f64::sqrt,
        "log" => f64::ln,
        "log10" => f64::log10,
        "exp" => f64::exp,
        "exp2" => f64::exp2,
        "sin" => f64::sin,
        "cos" => f64::cos,
        "tan" => f64::tan,
        "asin" => f64::asin,
        "acos" => f64::acos,
        "atan" => f64::atan,
        "sinh" => f64::sinh,
        "cosh" => f64::cosh,
        "tanh" => f64::tanh,
        "asinh" => f64::asinh,
        "acosh" => f64::acosh,
        "atanh" => f64::atanh,
        "floor" => f64::floor,
        "ceil" => f64::ceil,
    );
    map.insert("atan2", |_, args| checked("atan2", number(&args, 0, "atan2")?.atan2(number(&args, 1, "atan2")?)));
    // A unit-length copy of a vector. Its components may all have the same units (a vector of lengths, say),
    // which cancel: the result is always numbers.
    map.insert("normalize", |_, args| {
        let v = array(&args, 0, "normalize")?;
        let mut xs = Vec::with_capacity(v.len());
        let mut units: Option<&Value> = None;
        for x in v.iter() {
            let n = match x.as_number() {
                Some(n) => n,
                None => {
                    let (value, unit) = (x.get("value").and_then(Value::as_number), x.get("unit"));
                    match (value, unit) {
                        (Some(value), Some(unit)) if units.is_none_or(|u| u == unit) => {
                            units = Some(unit);
                            value
                        }
                        _ => return Err(error("@normalize: expected numbers or values with the same units")),
                    }
                }
            };
            xs.push(n);
        }
        if units.is_some() && xs.len() != v.len() {
            return Err(error("@normalize: mixed units"));
        }
        let norm = xs.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm == 0.0 {
            return Err(error("@normalize: zero vector"));
        }
        Ok(Value::array(xs.into_iter().map(|x| Value::number(x / norm)).collect()))
    });
    map.insert("hypot", |_, args| checked("hypot", number(&args, 0, "hypot")?.hypot(number(&args, 1, "hypot")?)));
}
