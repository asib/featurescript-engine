use super::{BuiltinFn, arg, array, index, map, number};
use crate::eval::error;
use crate::value::{Kind, Value, ValueMap};
use std::collections::HashMap;

pub fn register(m: &mut HashMap<&'static str, BuiltinFn>) {
    m.insert("size", |_, args| {
        let v = arg(&args, 0, "size")?;
        let n = match &v.kind {
            Kind::Array(a) => a.len(),
            Kind::Map(m) => m.len(),
            Kind::String(s) => s.chars().count(),
            _ => return Err(error(format!("@size: can't take the size of {}", v.type_description()))),
        };
        Ok(Value::number(n as f64))
    });
    // Keeps the array's type tag, which std relies on (`@resize(arr, 0) // keep type tag`).
    m.insert("resize", |_, args| {
        let a = array(&args, 0, "resize")?;
        let n = index(&args, 1, "resize")?;
        let fill = args.get(2).cloned().unwrap_or(Value::UNDEFINED);
        let mut items = a.as_ref().clone();
        items.resize(n, fill);
        Ok(Value::array(items).with_tag(args[0].tag))
    });
    m.insert("subArray", |_, args| {
        let a = array(&args, 0, "subArray")?;
        let start = index(&args, 1, "subArray")?;
        let end = if args.len() > 2 { index(&args, 2, "subArray")? } else { a.len() };
        if start > end || end > a.len() {
            return Err(error(format!("@subArray: range {start}..{end} out of bounds (size {})", a.len())));
        }
        // Keeps the type tag too: std divides a subarray of a weighted point, a `Vector`, by its weight.
        Ok(Value::array(a[start..end].to_vec()).with_tag(args[0].tag))
    });
    m.insert("concatenateArrays", |_, args| {
        let arrays = array(&args, 0, "concatenateArrays")?;
        let mut items = Vec::new();
        for a in arrays.iter() {
            match &a.kind {
                Kind::Array(part) => items.extend(part.iter().cloned()),
                _ => return Err(error("@concatenateArrays: every element must be an array")),
            }
        }
        Ok(Value::array(items))
    });
    m.insert("indexOf", |_, args| {
        let a = array(&args, 0, "indexOf")?;
        let value = arg(&args, 1, "indexOf")?;
        let start = if args.len() > 2 { index(&args, 2, "indexOf")? } else { 0 };
        let found = a.iter().skip(start).position(|v| v == value).map(|i| i + start);
        Ok(Value::number(found.map_or(-1.0, |i| i as f64)))
    });
    m.insert("keys", |_, args| Ok(Value::array(map(&args, 0, "keys")?.keys().cloned().collect())));
    m.insert("values", |_, args| Ok(Value::array(map(&args, 0, "values")?.values().cloned().collect())));
    m.insert("mergeMaps", |_, args| {
        let defaults = map(&args, 0, "mergeMaps")?;
        let overrides = map(&args, 1, "mergeMaps")?;
        let mut merged = defaults.as_ref().clone();
        for (k, v) in overrides.iter() {
            merged.insert(k.clone(), v.clone());
        }
        Ok(Value::map(merged).with_tag(args[0].tag))
    });
    m.insert("intersectMaps", |_, args| {
        let maps = array(&args, 0, "intersectMaps")?;
        let mut result: Option<ValueMap> = None;
        for v in maps.iter() {
            let Some(m) = v.as_map() else { return Err(error("@intersectMaps: every element must be a map")) };
            result = Some(match result {
                None => m.as_ref().clone(),
                Some(acc) => {
                    m.iter().filter(|(k, _)| acc.contains_key(k)).map(|(k, v)| (k.clone(), v.clone())).collect()
                }
            });
        }
        Ok(Value::map(result.unwrap_or_default()))
    });
    m.insert("reverse", |_, args| {
        let mut items = array(&args, 0, "reverse")?.as_ref().clone();
        items.reverse();
        Ok(Value::array(items))
    });
    m.insert("range", |_, args| {
        let count = index(&args, 2, "range")?;
        let from = arg(&args, 0, "range")?;
        let to = arg(&args, 1, "range")?;
        // Numbers, or ValueWithUnits maps { value, unit } sharing a unit.
        let (a, b, template) = match (&from.kind, &to.kind) {
            (Kind::Number(a), Kind::Number(b)) => (*a, *b, None),
            (Kind::Map(_), Kind::Map(_)) => {
                let value = |v: &Value| v.get("value").and_then(Value::as_number);
                match (value(from), value(to)) {
                    (Some(a), Some(b)) => (a, b, Some(from.clone())),
                    _ => return Err(error("@range: maps must have a numeric value")),
                }
            }
            _ => return Err(error("@range: arguments must be numbers or values with units")),
        };
        let mut items = Vec::with_capacity(count);
        for i in 0..count {
            let t = if count <= 1 { 0.0 } else { i as f64 / (count - 1) as f64 };
            let x = if i + 1 == count && count > 1 { b } else { a + (b - a) * t };
            items.push(match &template {
                None => Value::number(x),
                Some(template) => {
                    let mut entries = template.as_map().unwrap().as_ref().clone();
                    entries.insert(Value::string("value"), Value::number(x));
                    Value::map(entries).with_tag(template.tag)
                }
            });
        }
        Ok(Value::array(items))
    });
    // Indices that sort the numbers, treating values within `tolerance` as equal (keeping their order).
    m.insert("tolerantSort", |_, args| {
        let values = array(&args, 0, "tolerantSort")?;
        let tolerance = number(&args, 1, "tolerantSort")?;
        let mut keyed: Vec<(usize, f64)> = Vec::with_capacity(values.len());
        for (i, v) in values.iter().enumerate() {
            keyed.push((i, v.as_number().ok_or_else(|| error("@tolerantSort: values must be numbers"))?));
        }
        keyed.sort_by(
            |a, b| {
                if (a.1 - b.1).abs() <= tolerance { a.0.cmp(&b.0) } else { a.1.partial_cmp(&b.1).unwrap() }
            },
        );
        Ok(Value::array(keyed.into_iter().map(|(i, _)| Value::number(i as f64)).collect()))
    });
}
