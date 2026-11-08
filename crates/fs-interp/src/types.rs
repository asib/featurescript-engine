//! Type tags: enums and custom types, numbered in declaration order.
//!
//! The order matters because map keys sort by tag. Onshape orders tags by when their module first appears in the
//! import graph; the loader registers a module's types when it first reaches that module.
//!
//! The registry is per thread, and an [`crate::Engine`] resets it when created, so run one engine per thread.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub type TypeId = u32;

pub struct TypeInfo {
    pub name: Rc<str>,
    pub module: usize,
    pub kind: TypeKind,
}

pub enum TypeKind {
    Custom,
    Enum { values: Vec<Rc<str>>, index: HashMap<Rc<str>, usize> },
}

thread_local! {
    static TYPES: RefCell<Vec<TypeInfo>> = const { RefCell::new(Vec::new()) };
}

pub fn reset() {
    TYPES.with(|t| t.borrow_mut().clear());
}

pub fn register_custom(name: &str, module: usize) -> TypeId {
    TYPES.with(|t| {
        let mut t = t.borrow_mut();
        t.push(TypeInfo { name: name.into(), module, kind: TypeKind::Custom });
        (t.len() - 1) as TypeId
    })
}

pub fn register_enum(name: &str, module: usize, values: Vec<Rc<str>>) -> TypeId {
    let index = values.iter().enumerate().map(|(i, v)| (v.clone(), i)).collect();
    TYPES.with(|t| {
        let mut t = t.borrow_mut();
        t.push(TypeInfo { name: name.into(), module, kind: TypeKind::Enum { values, index } });
        (t.len() - 1) as TypeId
    })
}

/// The module that declares the type.
pub fn module(id: TypeId) -> Option<usize> {
    TYPES.with(|t| t.borrow().get(id as usize).map(|info| info.module))
}

pub fn name(id: TypeId) -> Rc<str> {
    TYPES.with(|t| t.borrow().get(id as usize).map_or_else(|| "?".into(), |info| info.name.clone()))
}

/// The first type registered with this name. std's types are registered before any module that imports std.
pub fn find(name: &str) -> Option<TypeId> {
    TYPES.with(|t| t.borrow().iter().position(|info| &*info.name == name).map(|i| i as TypeId))
}

pub fn is_enum(id: TypeId) -> bool {
    TYPES.with(|t| matches!(t.borrow().get(id as usize).map(|i| &i.kind), Some(TypeKind::Enum { .. })))
}

/// The declaration index of `value` in enum `id`, or `None` if `id` isn't an enum or doesn't contain it.
pub fn enum_index(id: TypeId, value: &str) -> Option<usize> {
    TYPES.with(|t| match &t.borrow().get(id as usize)?.kind {
        TypeKind::Enum { index, .. } => index.get(value).copied(),
        TypeKind::Custom => None,
    })
}

pub fn enum_values(id: TypeId) -> Vec<Rc<str>> {
    TYPES.with(|t| match t.borrow().get(id as usize).map(|i| &i.kind) {
        Some(TypeKind::Enum { values, .. }) => values.clone(),
        _ => Vec::new(),
    })
}
