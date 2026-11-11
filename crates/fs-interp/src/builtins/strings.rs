use super::{BuiltinFn, index, string};
use crate::eval::{R, error};
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
    // JSON to maps, arrays, strings, numbers and booleans; null is undefined, so a key set to it is left out.
    m.insert("parseJson", |_, args| {
        let text = string(&args, 0, "parseJson")?;
        let mut parser = Json { chars: text.chars().collect(), at: 0 };
        let value = parser.value()?;
        parser.space();
        if parser.at != parser.chars.len() {
            return Err(error("@parseJson: text after the value"));
        }
        Ok(value)
    });
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

/// A JSON reader for `@parseJson`.
struct Json {
    chars: Vec<char>,
    at: usize,
}

impl Json {
    fn space(&mut self) {
        while self.chars.get(self.at).is_some_and(|c| c.is_whitespace()) {
            self.at += 1;
        }
    }

    fn expect(&mut self, c: char) -> R<()> {
        self.space();
        if self.chars.get(self.at) == Some(&c) {
            self.at += 1;
            Ok(())
        } else {
            Err(error(format!("@parseJson: expected {c:?}")))
        }
    }

    fn word(&mut self, word: &str, value: Value) -> R<Value> {
        if self.chars[self.at..].iter().take(word.len()).copied().eq(word.chars()) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(error("@parseJson: not JSON"))
        }
    }

    fn value(&mut self) -> R<Value> {
        self.space();
        match self.chars.get(self.at).copied() {
            Some('{') => {
                self.at += 1;
                let mut entries = crate::value::ValueMap::new();
                self.space();
                if self.chars.get(self.at) == Some(&'}') {
                    self.at += 1;
                    return Ok(Value::map(entries));
                }
                loop {
                    self.space();
                    let key = self.string()?;
                    self.expect(':')?;
                    let value = self.value()?;
                    if !value.is_undefined() {
                        entries.insert(Value::string(key), value);
                    }
                    self.space();
                    match self.chars.get(self.at) {
                        Some(',') => self.at += 1,
                        Some('}') => {
                            self.at += 1;
                            return Ok(Value::map(entries));
                        }
                        _ => return Err(error("@parseJson: expected ',' or '}'")),
                    }
                }
            }
            Some('[') => {
                self.at += 1;
                let mut items = Vec::new();
                self.space();
                if self.chars.get(self.at) == Some(&']') {
                    self.at += 1;
                    return Ok(Value::array(items));
                }
                loop {
                    items.push(self.value()?);
                    self.space();
                    match self.chars.get(self.at) {
                        Some(',') => self.at += 1,
                        Some(']') => {
                            self.at += 1;
                            return Ok(Value::array(items));
                        }
                        _ => return Err(error("@parseJson: expected ',' or ']'")),
                    }
                }
            }
            Some('"') => Ok(Value::string(self.string()?)),
            Some('t') => self.word("true", Value::bool(true)),
            Some('f') => self.word("false", Value::bool(false)),
            Some('n') => self.word("null", Value::UNDEFINED),
            Some(c) if c == '-' || c.is_ascii_digit() => {
                let start = self.at;
                while self.chars.get(self.at).is_some_and(|c| c.is_ascii_digit() || "+-.eE".contains(*c)) {
                    self.at += 1;
                }
                let text: String = self.chars[start..self.at].iter().collect();
                text.parse::<f64>().map(Value::number).map_err(|_| error("@parseJson: bad number"))
            }
            _ => Err(error("@parseJson: not JSON")),
        }
    }

    fn string(&mut self) -> R<String> {
        if self.chars.get(self.at) != Some(&'"') {
            return Err(error("@parseJson: expected a string"));
        }
        self.at += 1;
        let mut out = String::new();
        loop {
            let Some(&c) = self.chars.get(self.at) else { return Err(error("@parseJson: unterminated string")) };
            self.at += 1;
            match c {
                '"' => return Ok(out),
                '\\' => {
                    let Some(&e) = self.chars.get(self.at) else { return Err(error("@parseJson: bad escape")) };
                    self.at += 1;
                    match e {
                        'n' => out.push('\n'),
                        't' => out.push('\t'),
                        'r' => out.push('\r'),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'u' => {
                            let hex: String = self.chars.get(self.at..self.at + 4).unwrap_or_default().iter().collect();
                            self.at += 4;
                            let code = u32::from_str_radix(&hex, 16).map_err(|_| error("@parseJson: bad escape"))?;
                            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                        }
                        other => out.push(other),
                    }
                }
                c => out.push(c),
            }
        }
    }
}
