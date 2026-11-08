//! Loading FeatureScript modules and resolving top-level names.
//!
//! Each file is a module. A name used in a module can refer to the module's own top-level declarations or to what
//! its imports export. A module exports its own `export` declarations plus everything exported by its `export
//! import`s. Functions with the same name form one overload set across modules.

use crate::types::{self, TypeId};
use crate::value::Value;
use fs_lang::ast::{self, ItemKind, OverloadableOp};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub type ModuleId = usize;

/// A top-level function, predicate or operator overload.
#[derive(Clone)]
pub struct FunctionRef {
    pub module: ModuleId,
    pub name: Rc<str>,
    pub function: Rc<ast::Function>,
    pub predicate: bool,
}

#[derive(Clone)]
pub enum Symbol {
    Function(FunctionRef),
    Const { module: ModuleId, item: usize },
    Type(TypeId),
    Enum(TypeId),
}

impl Symbol {
    fn same(&self, other: &Symbol) -> bool {
        match (self, other) {
            (Symbol::Function(a), Symbol::Function(b)) => Rc::ptr_eq(&a.function, &b.function),
            (Symbol::Const { module: m1, item: i1 }, Symbol::Const { module: m2, item: i2 }) => m1 == m2 && i1 == i2,
            (Symbol::Type(a), Symbol::Type(b)) | (Symbol::Enum(a), Symbol::Enum(b)) => a == b,
            _ => false,
        }
    }
}

pub struct Import {
    pub target: ModuleId,
    pub namespace: Option<Rc<str>>,
    pub exported: bool,
}

pub enum ConstState {
    Evaluating,
    Done(Value),
}

type SymbolTable = HashMap<Rc<str>, Vec<Symbol>>;

/// Cached lookups, keyed by optional namespace and name.
type LookupCache = HashMap<(Option<Rc<str>>, Rc<str>), Rc<[Symbol]>>;

enum ExportState {
    NotBuilt,
    /// Being built; an import cycle reaching it sees nothing exported.
    Building,
    Built(Rc<SymbolTable>),
}

pub struct Module {
    pub path: String,
    pub ast: Rc<ast::Module>,
    pub imports: Vec<Import>,
    /// Own top-level declarations, with whether each is exported.
    own: HashMap<Rc<str>, Vec<(Symbol, bool)>>,
    exports: RefCell<ExportState>,
    lookups: RefCell<LookupCache>,
    pub consts: RefCell<HashMap<usize, ConstState>>,
}

#[derive(Debug)]
pub struct LoadError(pub String);

pub struct Modules {
    pub list: Vec<Module>,
    by_path: HashMap<PathBuf, ModuleId>,
    std_dir: PathBuf,
    /// Operator overloads from every loaded module.
    pub operators: HashMap<OverloadableOp, Vec<FunctionRef>>,
}

impl Modules {
    pub fn new(std_dir: PathBuf) -> Modules {
        Modules { list: Vec::new(), by_path: HashMap::new(), std_dir, operators: HashMap::new() }
    }

    /// Where an import path points. `onshape/std/<file>` is in the vendored std; other paths are relative to the
    /// importing file.
    fn resolve_import(&self, importer: &Path, path: &str) -> PathBuf {
        match path.strip_prefix("onshape/std/") {
            Some(file) => self.std_dir.join(file),
            None => importer.parent().unwrap_or(Path::new(".")).join(path),
        }
    }

    pub fn load_file(&mut self, path: &Path) -> Result<ModuleId, LoadError> {
        let path = path.canonicalize().map_err(|e| LoadError(format!("{}: {e}", path.display())))?;
        if let Some(&id) = self.by_path.get(&path) {
            return Ok(id);
        }
        let source = std::fs::read_to_string(&path).map_err(|e| LoadError(format!("{}: {e}", path.display())))?;
        self.load_source(&path, &source)
    }

    /// Loads a module from source text. `path` names it and anchors relative imports.
    pub fn load_source(&mut self, path: &Path, source: &str) -> Result<ModuleId, LoadError> {
        let ast = fs_lang::parse_module(source)
            .map_err(|e| LoadError(format!("{}:{}", path.display(), e.describe(source))))?;
        let id = self.list.len();
        self.by_path.insert(path.to_path_buf(), id);
        let ast = Rc::new(ast);
        let mut own: HashMap<Rc<str>, Vec<(Symbol, bool)>> = HashMap::new();
        let mut add =
            |name: &str, symbol: Symbol, exported: bool| own.entry(name.into()).or_default().push((symbol, exported));

        // Register this module's types before loading its imports: tags are ordered by when their module is first
        // reached in the import graph.
        for (index, item) in ast.items.iter().enumerate() {
            match &item.kind {
                ItemKind::Type { name, .. } => {
                    add(&name.name, Symbol::Type(types::register_custom(&name.name, id)), item.exported)
                }
                ItemKind::Enum { name, values } => {
                    let values = values.iter().map(|v| Rc::from(v.name.name.as_str())).collect();
                    add(&name.name, Symbol::Enum(types::register_enum(&name.name, id, values)), item.exported);
                }
                ItemKind::Const { name, .. } => {
                    add(&name.name, Symbol::Const { module: id, item: index }, item.exported)
                }
                ItemKind::Function { name, function } | ItemKind::Predicate { name, function } => {
                    let predicate = matches!(item.kind, ItemKind::Predicate { .. });
                    let function = FunctionRef {
                        module: id,
                        name: name.name.as_str().into(),
                        function: function.clone(),
                        predicate,
                    };
                    add(&name.name, Symbol::Function(function), item.exported);
                }
                ItemKind::Operator { op, function } => {
                    let function = FunctionRef {
                        module: id,
                        name: operator_name(*op).into(),
                        function: function.clone(),
                        predicate: false,
                    };
                    self.operators.entry(*op).or_default().push(function);
                }
                ItemKind::Import { .. } => {}
            }
        }
        self.list.push(Module {
            path: path.display().to_string(),
            ast: ast.clone(),
            imports: Vec::new(),
            own,
            exports: RefCell::new(ExportState::NotBuilt),
            lookups: RefCell::new(HashMap::new()),
            consts: RefCell::new(HashMap::new()),
        });

        let mut imports = Vec::new();
        for item in &ast.items {
            if let ItemKind::Import { namespace, path: import_path, .. } = &item.kind {
                let target_path = self.resolve_import(path, import_path);
                let target = self.load_file(&target_path)?;
                imports.push(Import {
                    target,
                    namespace: namespace.as_ref().map(|n| n.name.as_str().into()),
                    exported: item.exported,
                });
            }
        }
        self.list[id].imports = imports;
        Ok(id)
    }

    /// Everything a module exports: its own exported declarations and its `export import`s' exports.
    pub fn exports(&self, id: ModuleId) -> Rc<SymbolTable> {
        match &*self.list[id].exports.borrow() {
            ExportState::Built(table) => return table.clone(),
            ExportState::Building => return Rc::new(HashMap::new()),
            ExportState::NotBuilt => {}
        }
        *self.list[id].exports.borrow_mut() = ExportState::Building;
        let module = &self.list[id];
        let mut table: SymbolTable = HashMap::new();
        for (name, symbols) in &module.own {
            for (symbol, exported) in symbols {
                if *exported {
                    merge(&mut table, name, symbol);
                }
            }
        }
        for import in &module.imports {
            if import.exported && import.namespace.is_none() {
                for (name, symbols) in self.exports(import.target).iter() {
                    for symbol in symbols {
                        merge(&mut table, name, symbol);
                    }
                }
            }
        }
        let table = Rc::new(table);
        *self.list[id].exports.borrow_mut() = ExportState::Built(table.clone());
        table
    }

    /// The symbols a name refers to in a module, optionally through an import namespace (`ns::name`).
    /// The exported functions a module itself declares under `name` (not those it imports).
    pub fn own_exported_functions(&self, id: ModuleId, name: &str) -> Vec<FunctionRef> {
        let Some(own) = self.list.get(id).and_then(|m| m.own.get(name)) else { return Vec::new() };
        own.iter()
            .filter_map(|(symbol, exported)| match symbol {
                Symbol::Function(f) if *exported => Some(f.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn lookup(&self, id: ModuleId, namespace: Option<&str>, name: &str) -> Rc<[Symbol]> {
        let key = (namespace.map(Rc::from), Rc::from(name));
        if let Some(found) = self.list[id].lookups.borrow().get(&key) {
            return found.clone();
        }
        let module = &self.list[id];
        let mut found: Vec<Symbol> = Vec::new();
        let mut push = |symbol: &Symbol| {
            if !found.iter().any(|s| s.same(symbol)) {
                found.push(symbol.clone());
            }
        };
        if namespace.is_none()
            && let Some(own) = module.own.get(name)
        {
            for (symbol, _) in own {
                push(symbol);
            }
        }
        for import in &module.imports {
            if import.namespace.as_deref() == namespace
                && let Some(symbols) = self.exports(import.target).get(name)
            {
                for symbol in symbols {
                    push(symbol);
                }
            }
        }
        let found: Rc<[Symbol]> = found.into();
        module.lookups.borrow_mut().insert(key, found.clone());
        found
    }
}

fn merge(table: &mut SymbolTable, name: &Rc<str>, symbol: &Symbol) {
    let entry = table.entry(name.clone()).or_default();
    if !entry.iter().any(|s| s.same(symbol)) {
        entry.push(symbol.clone());
    }
}

pub fn operator_name(op: OverloadableOp) -> &'static str {
    match op {
        OverloadableOp::Add => "operator+",
        OverloadableOp::Sub => "operator-",
        OverloadableOp::Mul => "operator*",
        OverloadableOp::Div => "operator/",
        OverloadableOp::Rem => "operator%",
        OverloadableOp::Pow => "operator^",
        OverloadableOp::Less => "operator<",
    }
}
