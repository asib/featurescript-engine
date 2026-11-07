//! FeatureScript values.
//!
//! Every value has one standard type (undefined, boolean, number, string, array, map, box, builtin, function) and at
//! most one type tag (an enum or custom type). Arrays and maps have value semantics: they are shared behind `Rc`
//! and copied on write. Boxes and builtins are references.
//!
//! Equality and the total order follow Onshape's guide (`FsDoc/relational.html`): values compare by type tag, then
//! standard type, then content. Tags sort by declaration order, which is the order type ids are handed out in.

use crate::types::{self, TypeId};
use fs_lang::ast;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

pub type ValueMap = BTreeMap<Value, Value>;

/// The variables a lambda captured, by value, innermost last.
pub type Captured = Rc<Vec<(Rc<str>, Value)>>;

#[derive(Clone)]
pub struct Value {
    pub tag: Option<TypeId>,
    pub kind: Kind,
}

#[derive(Clone)]
pub enum Kind {
    Undefined,
    Bool(bool),
    Number(f64),
    String(Rc<str>),
    Array(Rc<Vec<Value>>),
    Map(Rc<ValueMap>),
    Box(Rc<BoxCell>),
    Builtin(Rc<dyn Native>),
    Function(Rc<Function>),
}

/// The standard types, in the order the total order uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StandardType {
    Undefined,
    Boolean,
    Number,
    String,
    Array,
    Map,
    Box,
    Builtin,
    Function,
}

impl StandardType {
    pub fn from_name(name: &str) -> Option<StandardType> {
        Some(match name {
            "undefined" => StandardType::Undefined,
            "boolean" => StandardType::Boolean,
            "number" => StandardType::Number,
            "string" => StandardType::String,
            "array" => StandardType::Array,
            "map" => StandardType::Map,
            "box" => StandardType::Box,
            "builtin" => StandardType::Builtin,
            "function" => StandardType::Function,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            StandardType::Undefined => "undefined",
            StandardType::Boolean => "boolean",
            StandardType::Number => "number",
            StandardType::String => "string",
            StandardType::Array => "array",
            StandardType::Map => "map",
            StandardType::Box => "box",
            StandardType::Builtin => "builtin",
            StandardType::Function => "function",
        }
    }
}

thread_local! {
    static NEXT_SERIAL: Cell<u64> = const { Cell::new(1) };
}

/// A creation counter, used to order boxes, builtins and functions.
pub fn next_serial() -> u64 {
    NEXT_SERIAL.with(|n| {
        let serial = n.get();
        n.set(serial + 1);
        serial
    })
}

pub struct BoxCell {
    pub value: RefCell<Value>,
    pub serial: u64,
}

/// An opaque runtime object, such as a Context.
pub trait Native: Any {
    fn type_name(&self) -> &'static str;
    fn serial(&self) -> u64;
    fn as_any(&self) -> &dyn Any;
}

/// A function value: a lambda with the values it captured, a named top-level function (an overload set), or a
/// builtin function referred to by name.
pub struct Function {
    pub serial: u64,
    pub kind: FunctionKind,
}

pub enum FunctionKind {
    Lambda { function: Rc<ast::Function>, module: usize, captured: Captured },
    Named { name: Rc<str>, candidates: Rc<Vec<crate::module::FunctionRef>> },
}

impl Value {
    pub const UNDEFINED: Value = Value { tag: None, kind: Kind::Undefined };

    pub fn bool(b: bool) -> Value {
        Value { tag: None, kind: Kind::Bool(b) }
    }
    pub fn number(n: f64) -> Value {
        Value { tag: None, kind: Kind::Number(n) }
    }
    pub fn string(s: impl Into<Rc<str>>) -> Value {
        Value { tag: None, kind: Kind::String(s.into()) }
    }
    pub fn array(items: Vec<Value>) -> Value {
        Value { tag: None, kind: Kind::Array(Rc::new(items)) }
    }
    pub fn map(entries: ValueMap) -> Value {
        Value { tag: None, kind: Kind::Map(Rc::new(entries)) }
    }
    pub fn new_box(inner: Value) -> Value {
        Value { tag: None, kind: Kind::Box(Rc::new(BoxCell { value: RefCell::new(inner), serial: next_serial() })) }
    }
    pub fn builtin(native: Rc<dyn Native>) -> Value {
        Value { tag: None, kind: Kind::Builtin(native) }
    }
    pub fn function(kind: FunctionKind) -> Value {
        Value { tag: None, kind: Kind::Function(Rc::new(Function { serial: next_serial(), kind })) }
    }

    pub fn with_tag(mut self, tag: Option<TypeId>) -> Value {
        self.tag = tag;
        self
    }

    pub fn standard_type(&self) -> StandardType {
        match self.kind {
            Kind::Undefined => StandardType::Undefined,
            Kind::Bool(_) => StandardType::Boolean,
            Kind::Number(_) => StandardType::Number,
            Kind::String(_) => StandardType::String,
            Kind::Array(_) => StandardType::Array,
            Kind::Map(_) => StandardType::Map,
            Kind::Box(_) => StandardType::Box,
            Kind::Builtin(_) => StandardType::Builtin,
            Kind::Function(_) => StandardType::Function,
        }
    }

    pub fn is_undefined(&self) -> bool {
        matches!(self.kind, Kind::Undefined)
    }

    pub fn as_number(&self) -> Option<f64> {
        if let Kind::Number(n) = self.kind { Some(n) } else { None }
    }
    pub fn as_bool(&self) -> Option<bool> {
        if let Kind::Bool(b) = self.kind { Some(b) } else { None }
    }
    pub fn as_str(&self) -> Option<&str> {
        if let Kind::String(s) = &self.kind { Some(s) } else { None }
    }
    pub fn as_array(&self) -> Option<&Rc<Vec<Value>>> {
        if let Kind::Array(a) = &self.kind { Some(a) } else { None }
    }
    pub fn as_map(&self) -> Option<&Rc<ValueMap>> {
        if let Kind::Map(m) = &self.kind { Some(m) } else { None }
    }

    /// Looks up a string key in a map value.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_map()?.get(&Value::string(key))
    }

    /// A short description for error messages, like Onshape's "value is string".
    pub fn type_description(&self) -> String {
        match self.tag {
            Some(tag) => types::name(tag).to_string(),
            None => self.standard_type().name().to_string(),
        }
    }
}

// ----- Equality and order -----

impl PartialEq for Value {
    fn eq(&self, other: &Value) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Value {}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Value) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Value {
    fn cmp(&self, other: &Value) -> Ordering {
        // Untagged values come first; tags sort by declaration order (their ids).
        let by_tag = match (self.tag, other.tag) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(a), Some(b)) => a.cmp(&b),
        };
        by_tag.then_with(|| self.standard_type().cmp(&other.standard_type())).then_with(|| {
            match (&self.kind, &other.kind) {
                (Kind::Undefined, Kind::Undefined) => Ordering::Equal,
                (Kind::Bool(a), Kind::Bool(b)) => a.cmp(b),
                (Kind::Number(a), Kind::Number(b)) => {
                    // No NaN exists in FeatureScript; -0 and 0 are the same.
                    if a == b { Ordering::Equal } else { a.partial_cmp(b).unwrap_or(Ordering::Equal) }
                }
                (Kind::String(a), Kind::String(b)) => {
                    if let Some(tag) = self.tag
                        && let (Some(i), Some(j)) = (types::enum_index(tag, a), types::enum_index(tag, b))
                    {
                        return i.cmp(&j);
                    }
                    // By Unicode code points.
                    a.chars().cmp(b.chars())
                }
                (Kind::Array(a), Kind::Array(b)) => a.len().cmp(&b.len()).then_with(|| a.iter().cmp(b.iter())),
                (Kind::Map(a), Kind::Map(b)) => a.len().cmp(&b.len()).then_with(|| a.iter().cmp(b.iter())),
                (Kind::Box(a), Kind::Box(b)) => a.serial.cmp(&b.serial),
                (Kind::Builtin(a), Kind::Builtin(b)) => a.serial().cmp(&b.serial()),
                (Kind::Function(a), Kind::Function(b)) => function_cmp(a, b),
                _ => Ordering::Equal,
            }
        })
    }
}

fn function_cmp(a: &Function, b: &Function) -> Ordering {
    match (&a.kind, &b.kind) {
        (
            FunctionKind::Lambda { function: fa, captured: ca, .. },
            FunctionKind::Lambda { function: fb, captured: cb, .. },
        ) => {
            // Same definition and equal bound values are equal (FsDoc relational.html).
            let pa = Rc::as_ptr(fa) as usize;
            let pb = Rc::as_ptr(fb) as usize;
            pa.cmp(&pb).then_with(|| {
                let values = |c: &Vec<(Rc<str>, Value)>| c.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>();
                values(ca).cmp(&values(cb))
            })
        }
        (FunctionKind::Named { name: na, .. }, FunctionKind::Named { name: nb, .. }) => na.cmp(nb),
        (FunctionKind::Lambda { .. }, FunctionKind::Named { .. }) => Ordering::Less,
        (FunctionKind::Named { .. }, FunctionKind::Lambda { .. }) => Ordering::Greater,
    }
}

// ----- Text -----

/// Formats a number the way string concatenation does: integers without a decimal point.
pub fn format_number(n: f64) -> String {
    if n == n.trunc() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else if n.is_infinite() {
        if n > 0.0 { "inf".into() } else { "-inf".into() }
    } else {
        format!("{n}")
    }
}

/// The text `"" ~ value` produces.
pub fn concat_text(value: &Value) -> String {
    match &value.kind {
        Kind::Undefined => "undefined".into(),
        Kind::Bool(b) => b.to_string(),
        Kind::Number(n) => format_number(*n),
        Kind::String(s) => s.to_string(),
        Kind::Array(items) => {
            let parts: Vec<String> = items.iter().map(concat_text).collect();
            format!("[ {} ]", parts.join(" , "))
        }
        Kind::Map(entries) => {
            let parts: Vec<String> =
                entries.iter().map(|(k, v)| format!("{} : {}", concat_text(k), concat_text(v))).collect();
            format!("{{ {} }}", parts.join(" , "))
        }
        Kind::Box(cell) => format!("box({})", concat_text(&cell.value.borrow())),
        Kind::Builtin(native) => native.type_name().to_string(),
        Kind::Function(_) => "function".into(),
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match &self.kind {
            Kind::String(s) => format!("{s:?}"),
            _ => concat_text(self),
        };
        match self.tag {
            Some(tag) => write!(f, "{text} as {}", types::name(tag)),
            None => write!(f, "{text}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_follows_standard_types_then_content() {
        let mut values = vec![
            Value::string("b"),
            Value::number(2.0),
            Value::UNDEFINED,
            Value::array(vec![Value::number(1.0), Value::number(2.0)]),
            Value::bool(true),
            Value::array(vec![Value::number(9.0)]),
            Value::number(-0.0),
            Value::string("a"),
            Value::bool(false),
        ];
        values.sort();
        let text: Vec<String> = values.iter().map(|v| format!("{v:?}")).collect();
        assert_eq!(text, ["undefined", "false", "true", "0", "2", "\"a\"", "\"b\"", "[ 9 ]", "[ 1 , 2 ]"]);
        assert_eq!(Value::number(0.0), Value::number(-0.0));
    }

    #[test]
    fn boxes_are_equal_only_to_themselves() {
        let a = Value::new_box(Value::number(1.0));
        let b = Value::new_box(Value::number(1.0));
        assert_ne!(a, b);
        assert_eq!(a, a.clone());
    }

    #[test]
    fn number_text() {
        assert_eq!(format_number(3.0), "3");
        assert_eq!(format_number(-0.5), "-0.5");
        assert_eq!(format_number(0.1 + 0.2), "0.30000000000000004");
    }
}
