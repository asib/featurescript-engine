//! `@unpackQuery`: the compressed queries (`qCompressed`) Onshape writes into a Part Studio's FeatureScript (its
//! "Show code"), decoded as Onshape prints them (fixture batch 103 c002).
//!
//! The text after a leading `%` is one value, each written as a letter and its operands (counts and indices in
//! lowercase hexadecimal):
//!
//! - `S<length>$<text>`: a string. The length is pieces joined by `.`: a hexadecimal count of the text's characters
//!   it takes, or `-<index>`, the string table's string at that index; the string is its pieces joined by `.`
//!   (`S11.6$FeUNWBXWR3P4AUC_0wireOp` is `FeUNWBXWR3P4AUC_0.wireOp`, `S-d.7$imprint` then
//!   `FeUNWBXWR3P4AUC_0.imprint`). A string of several pieces puts its literal pieces in the string table before
//!   itself (batch 107: `-13` names a sketch entity id's first piece).
//! - `T` and `F`: true and false.
//! - `B<length>$<type name>` and then a value: the value with that type tag (`Ba$EntityType S4$EDGE`);
//!   `C<index>` and then a value: the value tagged with the string table's type name at that index.
//! - `M<count>`: a map of that many keys and values; `A<count>`: an array.
//! - `D<number>`: a number. `R<index>`: the value table's value at that index again.
//!
//! A query starting `&` is `&<length>$<base64>`: the zlib-compressed text of one starting `%`, that long (batch
//! 107). The string table holds every string, piece and type name in the order read; the value table every string, number, map
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
        let several = pieces.len() > 1;
        let mut parts = Vec::new();
        for piece in pieces {
            match piece {
                Piece::Count(n) => {
                    let end = self.at + n;
                    let text = self.text.get(self.at..end).ok_or_else(|| bad(self.text, self.at))?;
                    let text = String::from_utf8(text.to_vec()).map_err(|_| bad(self.text, self.at))?;
                    if several {
                        self.strings.push(text.as_str().into());
                    }
                    parts.push(text);
                    self.at = end;
                }
                Piece::Parent(index) => {
                    let s = self.strings.get(index).ok_or_else(|| bad(self.text, self.at))?;
                    parts.push(s.to_string());
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
            b'T' => Value::bool(true),
            b'F' => Value::bool(false),
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
    let inflated;
    let text = match text.strip_prefix('&') {
        Some(rest) => {
            let (_, data) = rest.split_once('$').ok_or_else(|| bad(text.as_bytes(), 0))?;
            inflated = base64(data).and_then(|z| zlib(&z)).ok_or_else(|| bad(text.as_bytes(), 0))?;
            inflated.as_slice()
        }
        None => text.as_bytes(),
    };
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

/// Standard base64, padding optional.
fn base64(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in text.bytes().filter(|&c| c != b'=') {
        acc = (acc << 6) | value(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// A zlib stream inflated (RFC 1950 around RFC 1951's deflate).
fn zlib(data: &[u8]) -> Option<Vec<u8>> {
    struct Bits<'a> {
        data: &'a [u8],
        at: usize,
        bit: u32,
    }
    impl Bits<'_> {
        fn get(&mut self, n: u32) -> Option<u32> {
            let mut v = 0;
            for i in 0..n {
                let byte = *self.data.get(self.at)?;
                v |= (((byte >> self.bit) & 1) as u32) << i;
                self.bit += 1;
                if self.bit == 8 {
                    self.bit = 0;
                    self.at += 1;
                }
            }
            Some(v)
        }
    }
    // A canonical Huffman code from code lengths: (counts per length, symbols in order).
    struct Code {
        counts: [u16; 16],
        symbols: Vec<u16>,
    }
    fn code(lengths: &[u8]) -> Code {
        let mut counts = [0u16; 16];
        for &l in lengths {
            counts[l as usize] += 1;
        }
        counts[0] = 0;
        let mut offsets = [0u16; 16];
        for l in 1..16 {
            offsets[l] = offsets[l - 1] + counts[l - 1];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (s, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbols[offsets[l as usize] as usize] = s as u16;
                offsets[l as usize] += 1;
            }
        }
        Code { counts, symbols }
    }
    fn decode(bits: &mut Bits, c: &Code) -> Option<u16> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for l in 1..16 {
            code |= bits.get(1)? as i32;
            let count = c.counts[l] as i32;
            if code - count < first {
                return c.symbols.get((index + code - first) as usize).copied();
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        None
    }
    const LENGTH_BASE: [u16; 29] =
        [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
    const LENGTH_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
    const DIST_BASE: [u16; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145,
        8193, 12289, 16385, 24577,
    ];
    const DIST_EXTRA: [u8; 30] =
        [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
    if data.len() < 2 || data[0] & 0x0f != 8 {
        return None;
    }
    let mut bits = Bits { data, at: 2, bit: 0 };
    let mut out: Vec<u8> = Vec::new();
    loop {
        let last = bits.get(1)?;
        match bits.get(2)? {
            0 => {
                if bits.bit != 0 {
                    bits.bit = 0;
                    bits.at += 1;
                }
                let len = u16::from_le_bytes([*data.get(bits.at)?, *data.get(bits.at + 1)?]) as usize;
                bits.at += 4;
                out.extend_from_slice(data.get(bits.at..bits.at + len)?);
                bits.at += len;
            }
            kind @ (1 | 2) => {
                let (lit, dist) = if kind == 1 {
                    let mut l = [8u8; 288];
                    l[144..256].fill(9);
                    l[256..280].fill(7);
                    (code(&l), code(&[5u8; 30]))
                } else {
                    let (hlit, hdist, hclen) = (bits.get(5)? as usize + 257, bits.get(5)? as usize + 1, bits.get(4)? as usize + 4);
                    const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
                    let mut cl = [0u8; 19];
                    for &i in ORDER.iter().take(hclen) {
                        cl[i] = bits.get(3)? as u8;
                    }
                    let clc = code(&cl);
                    let mut lengths = Vec::with_capacity(hlit + hdist);
                    while lengths.len() < hlit + hdist {
                        match decode(&mut bits, &clc)? {
                            s @ 0..=15 => lengths.push(s as u8),
                            16 => {
                                let prev = *lengths.last()?;
                                for _ in 0..3 + bits.get(2)? {
                                    lengths.push(prev);
                                }
                            }
                            17 => lengths.extend(std::iter::repeat_n(0, 3 + bits.get(3)? as usize)),
                            _ => lengths.extend(std::iter::repeat_n(0, 11 + bits.get(7)? as usize)),
                        }
                    }
                    (code(&lengths[..hlit]), code(&lengths[hlit..]))
                };
                loop {
                    match decode(&mut bits, &lit)? {
                        s @ 0..=255 => out.push(s as u8),
                        256 => break,
                        s => {
                            let i = (s - 257) as usize;
                            let len = *LENGTH_BASE.get(i)? as usize + bits.get(*LENGTH_EXTRA.get(i)? as u32)? as usize;
                            let d = decode(&mut bits, &dist)? as usize;
                            let back = *DIST_BASE.get(d)? as usize + bits.get(*DIST_EXTRA.get(d)? as u32)? as usize;
                            if back > out.len() {
                                return None;
                            }
                            for _ in 0..len {
                                out.push(out[out.len() - back]);
                            }
                        }
                    }
                }
            }
            _ => return None,
        }
        if last == 1 {
            return Some(out);
        }
    }
}
