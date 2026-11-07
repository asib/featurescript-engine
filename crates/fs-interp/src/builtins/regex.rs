//! A small backtracking regular expression engine for the regex builtins.
//!
//! It covers the Java-style syntax std uses: literals and escapes, `.`, character classes with ranges and
//! negation, `\d \D \s \S \w \W`, anchors `^ $`, word boundaries `\b \B`, capturing and non-capturing groups,
//! alternation, and greedy or lazy quantifiers `* + ? {n} {n,} {n,m}`.

use super::{BuiltinFn, index, map_value, string};
use crate::eval::{R, error};
use crate::value::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
enum Node {
    Char(char),
    Any,
    Class { negated: bool, items: Vec<ClassItem> },
    Start,
    End,
    WordBoundary(bool),
    Group { capture: Option<usize>, node: Box<Node> },
    Concat(Vec<Node>),
    Alt(Vec<Node>),
    Repeat { node: Box<Node>, min: usize, max: Option<usize>, greedy: bool },
}

#[derive(Debug, Clone)]
enum ClassItem {
    Range(char, char),
    Digit(bool),
    Space(bool),
    Word(bool),
}

pub struct Regex {
    node: Node,
    groups: usize,
}

struct Parser<'a> {
    chars: Vec<char>,
    pos: usize,
    groups: usize,
    source: &'a str,
}

impl Parser<'_> {
    fn err(&self, message: &str) -> crate::eval::Flow {
        error(format!("bad regular expression {:?}: {message}", self.source))
    }
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
    fn alt(&mut self) -> R<Node> {
        let mut options = vec![self.concat()?];
        while self.peek() == Some('|') {
            self.pos += 1;
            options.push(self.concat()?);
        }
        Ok(if options.len() == 1 { options.pop().unwrap() } else { Node::Alt(options) })
    }
    fn concat(&mut self) -> R<Node> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom()?;
            items.push(self.quantified(atom)?);
        }
        Ok(Node::Concat(items))
    }
    fn quantified(&mut self, atom: Node) -> R<Node> {
        let (min, max) = match self.peek() {
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('?') => (0, Some(1)),
            Some('{') => {
                let close =
                    self.chars[self.pos..].iter().position(|&c| c == '}').ok_or_else(|| self.err("unclosed {"))?;
                let body: String = self.chars[self.pos + 1..self.pos + close].iter().collect();
                let parse = |s: &str| s.trim().parse::<usize>().map_err(|_| self.err("bad {n,m}"));
                let (min, max) = match body.split_once(',') {
                    None => {
                        let n = parse(&body)?;
                        (n, Some(n))
                    }
                    Some((a, b)) if b.trim().is_empty() => (parse(a)?, None),
                    Some((a, b)) => (parse(a)?, Some(parse(b)?)),
                };
                self.pos += close;
                (min, max)
            }
            _ => return Ok(atom),
        };
        self.pos += 1;
        let greedy = if self.peek() == Some('?') {
            self.pos += 1;
            false
        } else {
            true
        };
        Ok(Node::Repeat { node: Box::new(atom), min, max, greedy })
    }
    fn atom(&mut self) -> R<Node> {
        let c = self.peek().unwrap();
        self.pos += 1;
        Ok(match c {
            '.' => Node::Any,
            '^' => Node::Start,
            '$' => Node::End,
            '(' => {
                let capture = if self.chars[self.pos..].starts_with(&['?', ':']) {
                    self.pos += 2;
                    None
                } else {
                    self.groups += 1;
                    Some(self.groups)
                };
                let node = self.alt()?;
                if self.peek() != Some(')') {
                    return Err(self.err("unclosed group"));
                }
                self.pos += 1;
                Node::Group { capture, node: Box::new(node) }
            }
            '[' => self.class()?,
            '\\' => self.escape(false)?,
            c => Node::Char(c),
        })
    }
    fn escape(&mut self, in_class: bool) -> R<Node> {
        let c = self.peek().ok_or_else(|| self.err("trailing backslash"))?;
        self.pos += 1;
        let class = |item| Node::Class { negated: false, items: vec![item] };
        Ok(match c {
            'd' => class(ClassItem::Digit(true)),
            'D' => class(ClassItem::Digit(false)),
            's' => class(ClassItem::Space(true)),
            'S' => class(ClassItem::Space(false)),
            'w' => class(ClassItem::Word(true)),
            'W' => class(ClassItem::Word(false)),
            'b' if !in_class => Node::WordBoundary(true),
            'B' if !in_class => Node::WordBoundary(false),
            'n' => Node::Char('\n'),
            't' => Node::Char('\t'),
            'r' => Node::Char('\r'),
            'f' => Node::Char('\u{c}'),
            c => Node::Char(c),
        })
    }
    fn class(&mut self) -> R<Node> {
        let negated = self.peek() == Some('^');
        if negated {
            self.pos += 1;
        }
        let mut items = Vec::new();
        let mut first = true;
        loop {
            let c = self.peek().ok_or_else(|| self.err("unclosed ["))?;
            if c == ']' && !first {
                self.pos += 1;
                break;
            }
            first = false;
            self.pos += 1;
            let start = if c == '\\' {
                match self.escape(true)? {
                    Node::Char(ch) => ch,
                    Node::Class { items: inner, .. } => {
                        items.extend(inner);
                        continue;
                    }
                    _ => unreachable!(),
                }
            } else {
                c
            };
            if self.peek() == Some('-') && self.chars.get(self.pos + 1).is_some_and(|&n| n != ']') {
                self.pos += 1;
                let mut end = self.peek().unwrap();
                self.pos += 1;
                if end == '\\' {
                    end = match self.escape(true)? {
                        Node::Char(ch) => ch,
                        _ => return Err(self.err("bad range")),
                    };
                }
                items.push(ClassItem::Range(start, end));
            } else {
                items.push(ClassItem::Range(start, start));
            }
        }
        Ok(Node::Class { negated, items })
    }
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

impl ClassItem {
    fn matches(&self, c: char) -> bool {
        match *self {
            ClassItem::Range(a, b) => a <= c && c <= b,
            ClassItem::Digit(yes) => c.is_ascii_digit() == yes,
            ClassItem::Space(yes) => matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}') == yes,
            ClassItem::Word(yes) => is_word(c) == yes,
        }
    }
}

type Caps = Vec<Option<(usize, usize)>>;

impl Regex {
    pub fn new(source: &str) -> R<Regex> {
        let mut parser = Parser { chars: source.chars().collect(), pos: 0, groups: 0, source };
        let node = parser.alt()?;
        if parser.pos != parser.chars.len() {
            return Err(parser.err("unbalanced )"));
        }
        Ok(Regex { node, groups: parser.groups })
    }

    /// Tries to match starting exactly at `start`; returns the end and the captures.
    fn match_at(&self, text: &[char], start: usize, whole: bool) -> Option<(usize, Caps)> {
        let mut caps: Caps = vec![None; self.groups + 1];
        let mut end = None;
        let matched = m(&self.node, text, start, &mut caps, &mut |pos, _| {
            if whole && pos != text.len() {
                return false;
            }
            end = Some(pos);
            true
        });
        if matched {
            let end = end.unwrap();
            caps[0] = Some((start, end));
            Some((end, caps))
        } else {
            None
        }
    }

    pub fn matches_whole(&self, text: &[char]) -> Option<Caps> {
        self.match_at(text, 0, true).map(|(_, caps)| caps)
    }

    pub fn find(&self, text: &[char], from: usize) -> Option<(usize, usize, Caps)> {
        (from..=text.len()).find_map(|start| self.match_at(text, start, false).map(|(end, caps)| (start, end, caps)))
    }
}

/// Matches `node` at `pos`, then calls `k` with the end position; backtracks if `k` fails.
fn m(node: &Node, text: &[char], pos: usize, caps: &mut Caps, k: &mut dyn FnMut(usize, &mut Caps) -> bool) -> bool {
    match node {
        Node::Char(c) => text.get(pos) == Some(c) && k(pos + 1, caps),
        Node::Any => text.get(pos).is_some_and(|&c| c != '\n' && c != '\r') && k(pos + 1, caps),
        Node::Class { negated, items } => {
            text.get(pos).is_some_and(|&c| items.iter().any(|i| i.matches(c)) != *negated) && k(pos + 1, caps)
        }
        Node::Start => pos == 0 && k(pos, caps),
        Node::End => pos == text.len() && k(pos, caps),
        Node::WordBoundary(yes) => {
            let before = pos > 0 && is_word(text[pos - 1]);
            let after = pos < text.len() && is_word(text[pos]);
            ((before != after) == *yes) && k(pos, caps)
        }
        Node::Group { capture, node } => {
            let capture = *capture;
            m(node, text, pos, caps, &mut |end, caps| {
                let saved = capture.map(|g| caps[g]);
                if let Some(g) = capture {
                    caps[g] = Some((pos, end));
                }
                if k(end, caps) {
                    return true;
                }
                if let (Some(g), Some(old)) = (capture, saved) {
                    caps[g] = old;
                }
                false
            })
        }
        Node::Concat(items) => concat(items, text, pos, caps, k),
        Node::Alt(options) => options.iter().any(|o| m(o, text, pos, caps, k)),
        Node::Repeat { node, min, max, greedy } => repeat(node, *min, *max, *greedy, 0, text, pos, caps, k),
    }
}

fn concat(
    items: &[Node],
    text: &[char],
    pos: usize,
    caps: &mut Caps,
    k: &mut dyn FnMut(usize, &mut Caps) -> bool,
) -> bool {
    match items.split_first() {
        None => k(pos, caps),
        Some((first, rest)) => m(first, text, pos, caps, &mut |next, caps| concat(rest, text, next, caps, k)),
    }
}

#[allow(clippy::too_many_arguments)]
fn repeat(
    node: &Node,
    min: usize,
    max: Option<usize>,
    greedy: bool,
    count: usize,
    text: &[char],
    pos: usize,
    caps: &mut Caps,
    k: &mut dyn FnMut(usize, &mut Caps) -> bool,
) -> bool {
    let can_stop = count >= min;
    // Not identical: the order decides which alternative is tried first.
    #[allow(clippy::if_same_then_else)]
    if greedy {
        more(node, min, max, greedy, count, text, pos, caps, k) || (can_stop && k(pos, caps))
    } else {
        (can_stop && k(pos, caps)) || more(node, min, max, greedy, count, text, pos, caps, k)
    }
}

/// One more iteration of a repeat, if allowed.
#[allow(clippy::too_many_arguments)]
fn more(
    node: &Node,
    min: usize,
    max: Option<usize>,
    greedy: bool,
    count: usize,
    text: &[char],
    pos: usize,
    caps: &mut Caps,
    k: &mut dyn FnMut(usize, &mut Caps) -> bool,
) -> bool {
    max.is_none_or(|max| count < max)
        && m(node, text, pos, caps, &mut |next, caps| {
            // A zero-width iteration can't make progress; stop repeating.
            next != pos && repeat(node, min, max, greedy, count + 1, text, next, caps, k)
        })
}

/// Expands `$n` in a replacement; backslashes are literal (see `docs/FINDINGS.md`).
fn expand(replacement: &str, text: &[char], caps: &Caps) -> String {
    let mut out = String::new();
    let chars: Vec<char> = replacement.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
            let g = chars[i + 1].to_digit(10).unwrap() as usize;
            if let Some(Some((a, b))) = caps.get(g) {
                out.extend(&text[*a..*b]);
            }
            i += 2;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

pub fn register(map: &mut HashMap<&'static str, BuiltinFn>) {
    map.insert("match", |_, args| {
        let s = string(&args, 0, "match")?;
        let regex = Regex::new(string(&args, 1, "match")?)?;
        let text: Vec<char> = s.chars().collect();
        Ok(match regex.matches_whole(&text) {
            Some(caps) => {
                let captures = caps
                    .iter()
                    .map(|c| match c {
                        Some((a, b)) => Value::string(text[*a..*b].iter().collect::<String>()),
                        None => Value::string(""),
                    })
                    .collect();
                map_value(vec![("hasMatch", Value::bool(true)), ("captures", Value::array(captures))])
            }
            None => map_value(vec![("hasMatch", Value::bool(false)), ("captures", Value::array(vec![]))]),
        })
    });
    map.insert("replace", |_, args| {
        let text: Vec<char> = string(&args, 0, "replace")?.chars().collect();
        let regex = Regex::new(string(&args, 1, "replace")?)?;
        let replacement = string(&args, 2, "replace")?;
        let mut out = String::new();
        let mut pos = 0;
        while let Some((start, end, caps)) = regex.find(&text, pos) {
            out.extend(&text[pos..start]);
            out.push_str(&expand(replacement, &text, &caps));
            if end == start {
                if start < text.len() {
                    out.push(text[start]);
                }
                pos = start + 1;
            } else {
                pos = end;
            }
            if pos > text.len() {
                break;
            }
        }
        if pos <= text.len() {
            out.extend(&text[pos..]);
        }
        Ok(Value::string(out))
    });
    map.insert("indexOfRegexp", |_, args| {
        let text: Vec<char> = string(&args, 0, "indexOfRegexp")?.chars().collect();
        let regex = Regex::new(string(&args, 1, "indexOfRegexp")?)?;
        let from = if args.len() > 2 { index(&args, 2, "indexOfRegexp")? } else { 0 };
        Ok(Value::number(regex.find(&text, from).map_or(-1.0, |(start, _, _)| start as f64)))
    });
    // Java's String.split: trailing empty pieces are dropped.
    map.insert("splitByRegexp", |_, args| {
        let text: Vec<char> = string(&args, 0, "splitByRegexp")?.chars().collect();
        let regex = Regex::new(string(&args, 1, "splitByRegexp")?)?;
        let mut pieces = Vec::new();
        let mut pos = 0;
        let mut search = 0;
        while let Some((start, end, _)) = regex.find(&text, search) {
            if end == start {
                search = start + 1;
                if search > text.len() {
                    break;
                }
                continue;
            }
            pieces.push(text[pos..start].iter().collect::<String>());
            pos = end;
            search = end;
        }
        pieces.push(text[pos..].iter().collect::<String>());
        while pieces.last().is_some_and(String::is_empty) {
            pieces.pop();
        }
        Ok(Value::array(pieces.into_iter().map(Value::string).collect()))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn whole(pattern: &str, text: &str) -> Option<Vec<String>> {
        let t: Vec<char> = text.chars().collect();
        Regex::new(pattern)
            .ok()?
            .matches_whole(&t)
            .map(|caps| caps.iter().map(|c| c.map_or(String::new(), |(a, b)| t[a..b].iter().collect())).collect())
    }

    #[test]
    fn std_patterns() {
        assert_eq!(whole("(.+)(\\[[0-9]+\\]\\.)(.*)", "a.b[12].c").unwrap(), ["a.b[12].c", "a.b", "[12].", "c"]);
        assert!(whole("[0-9a-fA-F]{24}", "0123456789abcdef01234567").is_some());
        assert!(whole("[0-9a-fA-F]{24}", "0123456789abcdef0123456").is_none());
        assert!(whole("\\*?[a-zA-Z0-9_.+/\\-]+", "*abc-1/2").is_some());
        let pitch =
            whole("(\\d+(?:\\.\\d+)?(?:\\s+\\d+\\/\\d+)?)\\s*(tpi|mm)(\\s*\\([A-Za-z]+\\))?", "1.5 mm").unwrap();
        assert_eq!(pitch[1], "1.5");
        assert_eq!(pitch[2], "mm");
        assert!(whole(".*[Ss]traight tap.*", "M6 straight tap x").is_some());
        assert!(whole("a+?b", "aaab").is_some());
    }

    #[test]
    fn find_and_word_boundaries() {
        let r = Regex::new("(?:meter|mm)\\b").unwrap();
        let t: Vec<char> = "5 mm x".chars().collect();
        assert_eq!(r.find(&t, 0).map(|(a, b, _)| (a, b)), Some((2, 4)));
        let t: Vec<char> = "5 mmx".chars().collect();
        assert_eq!(r.find(&t, 0), None);
    }
}
