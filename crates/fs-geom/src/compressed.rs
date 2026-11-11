//! `@unpackQuery`: the compressed queries (`qCompressed`) Onshape writes into a Part Studio's FeatureScript (its
//! "Show code"), decoded as Onshape prints them (fixture batch 103 c002).
//!
//! The text after a leading `%` is one value, each written as a letter and its operands (counts and indices in
//! lowercase hexadecimal):
//!
//! - `S<length>$<text>`: a string. The length is pieces joined by `.`: a hexadecimal count of the text's characters
//!   it takes, or `-<index>`, the string that many strings back in the string table without its last `.` piece; the
//!   string is its pieces joined by `.` (`S11.6$FeUNWBXWR3P4AUC_0wireOp` is `FeUNWBXWR3P4AUC_0.wireOp`,
//!   `S-d.7$imprint` then `FeUNWBXWR3P4AUC_0.imprint`).
//! - `B<length>$<type name>` and then a value: the value with that type tag (`Ba$EntityType S4$EDGE`);
//!   `C<index>` and then a value: the value tagged with the string table's type name at that index.
//! - `M<count>`: a map of that many keys and values; `A<count>`: an array.
//! - `D<number>`: a number. `R<index>`: the value table's value at that index again.
//!
//! The string table holds every string and type name in the order read; the value table every string, number, map
//! and array as it is completed, a tagged value in place of the value it tags. Every `Id` in the query is relative
//! to the root `Id` given (a query rooted at a feature's id finds that feature's own operations: c005).

use crate::values::Tags;
use fs_interp::Value;
use fs_interp::eval::{R, error};
use fs_interp::types;
use fs_interp::value::ValueMap;
use std::rc::Rc;

struct Decoder<'a> {
    text: &'a [u8],
    at: usize,
    strings: Vec<Rc<str>>,
    values: Vec<Value>,
    root: Vec<Rc<str>>,
    id_tag: Option<types::TypeId>,
}

fn bad(text: &[u8], at: usize) -> fs_interp::eval::Flow {
    error(format!("@unpackQuery: can't read the compressed query at {at}: {:?}", String::from_utf8_lossy(&text[at.min(text.len())..])))
}

impl Decoder<'_> {
    fn peek(&self) -> Option<u8> {
        self.text.get(self.at).copied()
    }

    fn hex(&mut self) -> R<usize> {
        let start = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) {
            self.at += 1;
        }
        let digits = std::str::from_utf8(&self.text[start..self.at]).map_err(|_| bad(self.text, start))?;
        usize::from_str_radix(digits, 16).map_err(|_| bad(self.text, start))
    }

    /// A string's length and text: `<pieces>$<text>`.
    fn string(&mut self) -> R<Rc<str>> {
        enum Piece {
            Count(usize),
            Parent(usize),
        }
        let mut pieces = Vec::new();
        loop {
            if self.peek() == Some(b'-') {
                self.at += 1;
                pieces.push(Piece::Parent(self.hex()?));
            } else {
                pieces.push(Piece::Count(self.hex()?));
            }
            match self.peek() {
                Some(b'.') => self.at += 1,
                Some(b'$') => {
                    self.at += 1;
                    break;
                }
                _ => return Err(bad(self.text, self.at)),
            }
        }
        let mut parts = Vec::new();
        for piece in pieces {
            match piece {
                Piece::Count(n) => {
                    let end = self.at + n;
                    let text = self.text.get(self.at..end).ok_or_else(|| bad(self.text, self.at))?;
                    parts.push(String::from_utf8(text.to_vec()).map_err(|_| bad(self.text, self.at))?);
                    self.at = end;
                }
                Piece::Parent(index) => {
                    let s = self.strings.get(index).ok_or_else(|| bad(self.text, self.at))?;
                    parts.push(s.rsplit_once('.').map_or(&**s, |(head, _)| head).to_string());
                }
            }
        }
        let s: Rc<str> = parts.join(".").into();
        self.strings.push(s.clone());
        Ok(s)
    }

    fn tagged(&mut self, name: Rc<str>) -> R<Value> {
        let tag = types::find(&name).ok_or_else(|| error(format!("@unpackQuery: no type {name}")))?;
        let mut value = self.value()?.with_tag(Some(tag));
        if Some(tag) == self.id_tag && !self.root.is_empty() {
            let mut parts: Vec<Value> = self.root.iter().map(|p| Value::string(p.clone())).collect();
            parts.extend(value.as_array().map(|a| a.iter().cloned().collect::<Vec<_>>()).unwrap_or_default());
            value = Value::array(parts).with_tag(Some(tag));
        }
        // The value table holds the tagged value in place of the one it tags.
        if let Some(last) = self.values.last_mut() {
            *last = value.clone();
        }
        Ok(value)
    }

    fn value(&mut self) -> R<Value> {
        let start = self.at;
        let letter = self.peek().ok_or_else(|| bad(self.text, start))?;
        self.at += 1;
        let value = match letter {
            b'S' => Value::string(self.string()?),
            b'B' => {
                let name = self.string()?;
                return self.tagged(name);
            }
            b'C' => {
                let index = self.hex()?;
                let name = self.strings.get(index).cloned().ok_or_else(|| bad(self.text, start))?;
                return self.tagged(name);
            }
            b'R' => {
                let index = self.hex()?;
                return self.values.get(index).cloned().ok_or_else(|| bad(self.text, start));
            }
            b'M' => {
                let count = self.hex()?;
                let mut entries = ValueMap::new();
                for _ in 0..count {
                    let key = self.value()?;
                    let value = self.value()?;
                    entries.insert(key, value);
                }
                Value::map(entries)
            }
            b'A' => {
                let count = self.hex()?;
                let items = (0..count).map(|_| self.value()).collect::<R<Vec<_>>>()?;
                Value::array(items)
            }
            b'D' => {
                let begin = self.at;
                while self.peek().is_some_and(|c| c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.' | b'e')) {
                    self.at += 1;
                }
                let text = std::str::from_utf8(&self.text[begin..self.at]).map_err(|_| bad(self.text, start))?;
                Value::number(text.parse::<f64>().map_err(|_| bad(self.text, start))?)
            }
            _ => return Err(bad(self.text, start)),
        };
        self.values.push(value.clone());
        Ok(value)
    }
}

/// Decodes a compressed query, its operation ids under `root`.
pub fn unpack(tags: &Tags, text: &str, root: &[Rc<str>]) -> R<Value> {
    let text = text.as_bytes();
    if text.first() != Some(&b'%') {
        return Err(fs_interp::eval::execution_error("not a compressed query"));
    }
    let mut decoder =
        Decoder { text, at: 1, strings: Vec::new(), values: Vec::new(), root: root.to_vec(), id_tag: Some(tags.id) };
    let value = decoder.value()?;
    if decoder.at != text.len() {
        return Err(bad(text, decoder.at));
    }
    Ok(value)
}

/// An operation id as the kernel's operations name them: each part of a decoded `Id` may hold several, joined by
/// `.` (`FeUNWBXWR3P4AUC_0.wireOp`).
pub fn operation_parts(id: &[Rc<str>]) -> Vec<Rc<str>> {
    id.iter().flat_map(|part| part.split('.').map(Rc::from).collect::<Vec<_>>()).collect()
}

pub fn register(m: &mut crate::GeomBuiltins) {
    m.insert("unpackQuery", |interp, args| {
        let tags = crate::tags(interp)?;
        let text = fs_interp::builtins::string(&args, 1, "unpackQuery")?;
        let root = crate::values::feature_id(fs_interp::builtins::arg(&args, 2, "unpackQuery")?, "qCompressed root")?;
        unpack(&tags, &text, &root)
    });
}
