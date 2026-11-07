use super::{BuiltinFn, index, string};
use crate::eval::error;
use crate::value::Value;
use std::collections::HashMap;

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

pub fn register(m: &mut HashMap<&'static str, BuiltinFn>) {
    m.insert("print", |interp, args| {
        let text = string(&args, 0, "print")?;
        interp.output.push_str(text);
        Ok(Value::UNDEFINED)
    });
    m.insert("length", |_, args| Ok(Value::number(string(&args, 0, "length")?.chars().count() as f64)));
    m.insert("substring", |_, args| {
        let s = chars(string(&args, 0, "substring")?);
        let start = index(&args, 1, "substring")?;
        let end = if args.len() > 2 { index(&args, 2, "substring")? } else { s.len() };
        if start > end || end > s.len() {
            return Err(error(format!("@substring: range {start}..{end} out of bounds (length {})", s.len())));
        }
        Ok(Value::string(s[start..end].iter().collect::<String>()))
    });
    m.insert("splitIntoCharacters", |_, args| {
        let s = string(&args, 0, "splitIntoCharacters")?;
        Ok(Value::array(s.chars().map(|c| Value::string(c.to_string())).collect()))
    });
    m.insert("startsWith", |_, args| {
        Ok(Value::bool(string(&args, 0, "startsWith")?.starts_with(string(&args, 1, "startsWith")?)))
    });
    m.insert("endsWith", |_, args| {
        Ok(Value::bool(string(&args, 0, "endsWith")?.ends_with(string(&args, 1, "endsWith")?)))
    });
    m.insert("indexOfString", |_, args| {
        let s = chars(string(&args, 0, "indexOfString")?);
        let needle = chars(string(&args, 1, "indexOfString")?);
        let start = if args.len() > 2 { index(&args, 2, "indexOfString")? } else { 0 };
        let found = (start..=s.len().saturating_sub(needle.len()))
            .find(|&i| i + needle.len() <= s.len() && s[i..i + needle.len()] == needle[..]);
        Ok(Value::number(found.map_or(-1.0, |i| i as f64)))
    });
    m.insert("repeatString", |_, args| {
        Ok(Value::string(string(&args, 0, "repeatString")?.repeat(index(&args, 1, "repeatString")?)))
    });
    m.insert("stringToNumber", |_, args| {
        let s = string(&args, 0, "stringToNumber")?;
        s.trim().parse::<f64>().map(Value::number).map_err(|_| error(format!("@stringToNumber: {s:?} is not a number")))
    });
}
