//! The tree-walking evaluator.

use crate::builtins::{self, BuiltinFn};
use crate::module::{ConstState, FunctionRef, ModuleId, Modules, Symbol, operator_name};
use crate::types::{self, TypeId};
use crate::value::{Captured, FunctionKind, Kind, StandardType, Value, ValueMap, concat_text};
use fs_lang::ast::{
    self, AssignOp, BinaryOp, DeclKind, Expr, ExprKind, FunctionBody, ItemKind, MapKey, OverloadableOp, Stmt, StmtKind,
    TypeName, UnaryOp,
};
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

/// Non-local control flow. Exceptions are values: language errors are strings, library errors are maps.
#[derive(Debug)]
pub enum Flow {
    Throw(Value),
    Return(Value),
    Break,
    Continue,
}

pub type R<T> = Result<T, Flow>;

pub fn error(message: impl Into<String>) -> Flow {
    Flow::Throw(Value::string(message.into()))
}

const MAX_CALL_DEPTH: usize = 2000;

/// A resolved type constraint.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TypeTest {
    Standard(StandardType),
    Tag(TypeId),
}

impl TypeTest {
    pub fn matches(self, value: &Value) -> bool {
        match self {
            TypeTest::Standard(st) => value.standard_type() == st,
            TypeTest::Tag(tag) => value.tag == Some(tag),
        }
    }

    /// Specificity for overload resolution: tag > standard type > none.
    fn rank(test: Option<TypeTest>) -> u8 {
        match test {
            Some(TypeTest::Tag(_)) => 2,
            Some(TypeTest::Standard(_)) => 1,
            None => 0,
        }
    }
}

struct Local {
    name: Rc<str>,
    value: Value,
    constant: bool,
}

pub struct Interp {
    pub modules: Modules,
    pub builtins: HashMap<&'static str, BuiltinFn>,
    /// Builtins that were called but aren't written yet, even if the error was caught.
    pub missing_builtins: BTreeSet<String>,
    /// Text passed to `@print`.
    pub output: String,
    /// Diagnostic only: let unwritten builtins return `undefined` instead of throwing, to see what else a run
    /// would reach. Results are meaningless in this mode.
    pub lenient_builtins: bool,
    /// Diagnostic only: print every exception a `try` statement catches to stderr.
    pub trace_catches: bool,
    locals: Vec<Local>,
    frame_base: usize,
    module: ModuleId,
    captured: Option<Captured>,
    depth: usize,
    enum_maps: HashMap<TypeId, Value>,
    param_types: HashMap<usize, Rc<Vec<Option<TypeTest>>>>,
    /// Per-engine state for builtins (contexts, timers and so on).
    pub state: builtins::State,
    /// The first predicate statement found false since the last precondition started, for its error message.
    failed_predicate: Option<String>,
}

enum Step {
    Key(Value),
}

impl Interp {
    pub fn new(modules: Modules) -> Interp {
        Interp {
            modules,
            builtins: builtins::registry(),
            missing_builtins: BTreeSet::new(),
            output: String::new(),
            lenient_builtins: false,
            trace_catches: false,
            failed_predicate: None,
            locals: Vec::new(),
            frame_base: 0,
            module: 0,
            captured: None,
            depth: 0,
            enum_maps: HashMap::new(),
            param_types: HashMap::new(),
            state: builtins::State::default(),
        }
    }

    pub fn current_module(&self) -> ModuleId {
        self.module
    }

    // ----- Names -----

    fn find_local(&self, name: &str) -> Option<Value> {
        for local in self.locals[self.frame_base..].iter().rev() {
            if &*local.name == name {
                return Some(local.value.clone());
            }
        }
        if let Some(captured) = &self.captured {
            for (n, v) in captured.iter().rev() {
                if &**n == name {
                    return Some(v.clone());
                }
            }
        }
        None
    }

    fn lookup_name(&mut self, path: &ast::Path) -> R<Value> {
        if path.namespace.is_none()
            && let Some(value) = self.find_local(&path.name.name)
        {
            return Ok(value);
        }
        let namespace = path.namespace.as_ref().map(|n| n.name.as_str());
        let symbols = self.modules.lookup(self.module, namespace, &path.name.name);
        self.symbol_value(&path.name.name, &symbols)
    }

    fn symbol_value(&mut self, name: &str, symbols: &[Symbol]) -> R<Value> {
        let mut functions = Vec::new();
        for symbol in symbols {
            match symbol {
                Symbol::Const { module, item } => return self.const_value(*module, *item),
                Symbol::Enum(id) => return Ok(self.enum_map(*id)),
                Symbol::Function(f) => functions.push(f.clone()),
                Symbol::Type(_) => {}
            }
        }
        // A named function can only be called: Onshape rejects using one as a value (batch 008's first build).
        if !functions.is_empty() {
            return Err(error(format!("Cannot reference function {name} as a variable.")));
        }
        Err(error(format!("Variable {name} not found")))
    }

    /// The value of a top-level constant, evaluated on first use.
    pub fn const_value(&mut self, module: ModuleId, item: usize) -> R<Value> {
        let ast = self.modules.list[module].ast.clone();
        let ItemKind::Const { name, ty, value } = &ast.items[item].kind else { unreachable!() };
        match self.modules.list[module].consts.borrow().get(&item) {
            Some(ConstState::Done(v)) => return Ok(v.clone()),
            Some(ConstState::Evaluating) => return Err(error(format!("Constant {} refers to itself", name.name))),
            None => {}
        }
        self.modules.list[module].consts.borrow_mut().insert(item, ConstState::Evaluating);
        let result = self.in_frame(module, None, |this| {
            let v = this.eval(value)?;
            if let Some(ty) = ty {
                let test = this.resolve_type(ty)?;
                if !test.matches(&v) {
                    return Err(error(format!("Constant {} is not {}", name.name, ty.path.name.name)));
                }
            }
            Ok(v)
        });
        match &result {
            Ok(v) => self.modules.list[module].consts.borrow_mut().insert(item, ConstState::Done(v.clone())),
            Err(_) => self.modules.list[module].consts.borrow_mut().remove(&item),
        };
        result
    }

    fn enum_map(&mut self, id: TypeId) -> Value {
        if let Some(map) = self.enum_maps.get(&id) {
            return map.clone();
        }
        let mut entries = ValueMap::new();
        for name in types::enum_values(id) {
            entries.insert(Value::string(name.clone()), Value::string(name).with_tag(Some(id)));
        }
        let map = Value::map(entries);
        self.enum_maps.insert(id, map.clone());
        map
    }

    /// Runs `f` in a fresh frame of `module`, seeing only `captured` and module-level names.
    fn in_frame<T>(&mut self, module: ModuleId, captured: Option<Captured>, f: impl FnOnce(&mut Self) -> R<T>) -> R<T> {
        if self.depth >= MAX_CALL_DEPTH {
            return Err(error("Maximum call depth exceeded"));
        }
        let saved_base = self.frame_base;
        let saved_module = self.module;
        let saved_captured = std::mem::replace(&mut self.captured, captured);
        let base = self.locals.len();
        self.frame_base = base;
        self.module = module;
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        self.locals.truncate(base);
        self.frame_base = saved_base;
        self.module = saved_module;
        self.captured = saved_captured;
        result
    }

    fn in_scope<T>(&mut self, f: impl FnOnce(&mut Self) -> R<T>) -> R<T> {
        let mark = self.locals.len();
        let result = f(self);
        self.locals.truncate(mark);
        result
    }

    fn declare(&mut self, name: &str, value: Value, constant: bool) {
        self.locals.push(Local { name: name.into(), value, constant });
    }

    // ----- Types -----

    pub fn resolve_type(&mut self, ty: &TypeName) -> R<TypeTest> {
        self.resolve_type_in(self.module, ty)
    }

    fn resolve_type_in(&mut self, module: ModuleId, ty: &TypeName) -> R<TypeTest> {
        let path = &ty.path;
        if path.namespace.is_none()
            && let Some(st) = StandardType::from_name(&path.name.name)
        {
            return Ok(TypeTest::Standard(st));
        }
        let namespace = path.namespace.as_ref().map(|n| n.name.as_str());
        for symbol in self.modules.lookup(module, namespace, &path.name.name).iter() {
            match symbol {
                Symbol::Type(id) | Symbol::Enum(id) => return Ok(TypeTest::Tag(*id)),
                _ => {}
            }
        }
        Err(error(format!("Type {} not found", path.name.name)))
    }

    fn param_types(&mut self, function: &FunctionRef) -> R<Rc<Vec<Option<TypeTest>>>> {
        let key = Rc::as_ptr(&function.function) as usize;
        if let Some(types) = self.param_types.get(&key) {
            return Ok(types.clone());
        }
        let mut types = Vec::new();
        for param in &function.function.params {
            types.push(match &param.ty {
                Some(ty) => Some(self.resolve_type_in(function.module, ty)?),
                None => None,
            });
        }
        let types = Rc::new(types);
        self.param_types.insert(key, types.clone());
        Ok(types)
    }

    fn cast(&mut self, value: Value, ty: &TypeName) -> R<Value> {
        match self.resolve_type(ty)? {
            TypeTest::Standard(st) => {
                if value.standard_type() != st {
                    return Err(error(format!("Cannot convert {} to {}", value.type_description(), st.name())));
                }
                Ok(value.with_tag(None))
            }
            TypeTest::Tag(tag) => {
                if types::is_enum(tag) {
                    let ok = value.as_str().is_some_and(|s| types::enum_index(tag, s).is_some());
                    if !ok {
                        return Err(error(format!("Cannot convert {:?} to {}", value, types::name(tag))));
                    }
                }
                Ok(value.with_tag(Some(tag)))
            }
        }
    }

    // ----- Calls -----

    pub fn call_value(&mut self, callee: &Value, args: Vec<Value>) -> R<Value> {
        let Kind::Function(function) = &callee.kind else {
            return Err(error(format!("Cannot call a value of type {}", callee.type_description())));
        };
        match &function.kind {
            FunctionKind::Lambda { function, module, captured } => {
                if function.params.len() != args.len() {
                    return Err(error(format!("Function with {} argument(s) not found", args.len())));
                }
                for (param, arg) in function.params.iter().zip(&args) {
                    if let Some(ty) = &param.ty {
                        let test = self.resolve_type_in(*module, ty)?;
                        if !test.matches(arg) {
                            return Err(error(format!(
                                "Argument {} is {}, not {}",
                                param.name.name,
                                arg.type_description(),
                                ty.path.name.name
                            )));
                        }
                    }
                }
                self.call_function(function, *module, Some(captured.clone()), args, false, "lambda")
            }
            FunctionKind::Named { name, candidates } => self.call_overloaded(name, candidates, args),
        }
    }

    pub fn call_overloaded(&mut self, name: &str, candidates: &[FunctionRef], args: Vec<Value>) -> R<Value> {
        let chosen = self.resolve_overload(name, candidates, &args)?;
        self.call_function(&chosen.function, chosen.module, None, args, chosen.predicate, name)
    }

    fn resolve_overload(&mut self, name: &str, candidates: &[FunctionRef], args: &[Value]) -> R<FunctionRef> {
        let mut matching: Vec<(&FunctionRef, Vec<u8>)> = Vec::new();
        for candidate in candidates {
            if candidate.function.params.len() != args.len() {
                continue;
            }
            let types = self.param_types(candidate)?;
            if types.iter().zip(args).all(|(test, arg)| test.is_none_or(|t| t.matches(arg))) {
                matching.push((candidate, types.iter().map(|t| TypeTest::rank(*t)).collect()));
            }
        }
        match matching.len() {
            0 => Err(error(format!("Function {name} with {} argument(s) not found", args.len()))),
            1 => Ok(matching[0].0.clone()),
            _ => {
                // The most specific overload is at least as specific as every other one in every argument.
                let best = matching
                    .iter()
                    .find(|(_, rank)| matching.iter().all(|(_, other)| rank.iter().zip(other).all(|(a, b)| a >= b)));
                match best {
                    Some((f, _)) => Ok((*f).clone()),
                    None => Err(error(format!("Ambiguous call to {name} with {} argument(s)", args.len()))),
                }
            }
        }
    }

    fn call_function(
        &mut self,
        function: &Rc<ast::Function>,
        module: ModuleId,
        captured: Option<Captured>,
        args: Vec<Value>,
        predicate: bool,
        name: &str,
    ) -> R<Value> {
        let function = function.clone();
        self.in_frame(module, captured, |this| {
            for (param, arg) in function.params.iter().zip(args) {
                this.declare(&param.name.name, arg, false);
            }
            if let Some(precondition) = &function.precondition {
                this.failed_predicate = None;
                if !this.exec_predicate(precondition)? {
                    let detail = this.failed_predicate.take().map(|s| format!(" ({s})")).unwrap_or_default();
                    return Err(error(format!("Precondition of {name} failed{detail}")));
                }
            }
            let value = match &function.body {
                FunctionBody::Block(block) if predicate => Value::bool(this.exec_predicate_block(block)?),
                FunctionBody::Block(block) => match this.exec_block(block) {
                    Ok(()) => Value::UNDEFINED,
                    Err(Flow::Return(v)) => v,
                    Err(Flow::Break | Flow::Continue) => return Err(error("break or continue outside a loop")),
                    Err(e) => return Err(e),
                },
                FunctionBody::Expr(expr) => this.eval(expr)?,
            };
            if let Some(returns) = &function.returns {
                let test = this.resolve_type(returns)?;
                if !test.matches(&value) {
                    return Err(error(format!(
                        "{name} returned {}, not {}",
                        value.type_description(),
                        returns.path.name.name
                    )));
                }
            }
            Ok(value)
        })
    }

    fn call_by_name(&mut self, path: &ast::Path, args: Vec<Value>) -> R<Value> {
        if path.namespace.is_none()
            && let Some(value) = self.find_local(&path.name.name)
        {
            return self.call_value(&value, args);
        }
        let namespace = path.namespace.as_ref().map(|n| n.name.as_str());
        let symbols = self.modules.lookup(self.module, namespace, &path.name.name);
        let functions: Vec<FunctionRef> =
            symbols.iter().filter_map(|s| if let Symbol::Function(f) = s { Some(f.clone()) } else { None }).collect();
        if !functions.is_empty() {
            return self.call_overloaded(&path.name.name, &functions, args);
        }
        for symbol in symbols.iter() {
            if let Symbol::Const { module, item } = symbol {
                let value = self.const_value(*module, *item)?;
                return self.call_value(&value, args);
            }
        }
        // Like Onshape, a call also finds the overloads exported by the module declaring each argument's type:
        // std's `toString(value is map)` prints a `Vector` in it with vector.fs's `toString` (batch 011).
        if namespace.is_none() {
            for arg in &args {
                let Some(module) = arg.tag.and_then(types::module) else { continue };
                for f in self.modules.own_exported_functions(module, &path.name.name) {
                    if !functions.iter().any(|g| Rc::ptr_eq(&g.function, &f.function)) {
                        functions.push(f);
                    }
                }
            }
        }
        Err(error(format!("Function {} with {} argument(s) not found", path.name.name, args.len())))
    }

    pub fn call_builtin(&mut self, name: &str, args: Vec<Value>) -> R<Value> {
        match self.builtins.get(name) {
            Some(f) => f(self, args),
            None => {
                self.missing_builtins.insert(name.to_string());
                if self.lenient_builtins {
                    return Ok(Value::UNDEFINED);
                }
                Err(error(format!("not implemented: @{name}")))
            }
        }
    }

    // ----- Operators -----

    fn overload(&mut self, op: OverloadableOp, args: &[&Value]) -> R<Option<FunctionRef>> {
        let Some(candidates) = self.modules.operators.get(&op).cloned() else { return Ok(None) };
        let args: Vec<Value> = args.iter().map(|v| (*v).clone()).collect();
        match self.resolve_overload(operator_name(op), &candidates, &args) {
            Ok(f) => Ok(Some(f)),
            Err(_) => Ok(None),
        }
    }

    pub fn arithmetic(&mut self, op: OverloadableOp, a: Value, b: Value) -> R<Value> {
        if (a.tag.is_some() || b.tag.is_some())
            && let Some(f) = self.overload(op, &[&a, &b])?
        {
            return self.call_function(&f.function, f.module, None, vec![a, b], false, operator_name(op));
        }
        let (Some(x), Some(y)) = (a.as_number(), b.as_number()) else {
            return Err(error(format!(
                "{} can't be applied to {} and {}",
                operator_symbol(op),
                a.type_description(),
                b.type_description()
            )));
        };
        let result = match op {
            OverloadableOp::Add => x + y,
            OverloadableOp::Sub => x - y,
            OverloadableOp::Mul => x * y,
            OverloadableOp::Div => x / y,
            // The result takes the sign of the divisor.
            OverloadableOp::Rem => x - y * (x / y).floor(),
            OverloadableOp::Pow => x.powf(y),
            OverloadableOp::Less => unreachable!(),
        };
        if result.is_nan() {
            return Err(error(format!("Invalid numeric operation: {x} {} {y}", operator_symbol(op))));
        }
        Ok(Value::number(result))
    }

    pub fn less(&mut self, a: Value, b: Value) -> R<bool> {
        if a.tag.is_some() || b.tag.is_some() {
            if let Some(f) = self.overload(OverloadableOp::Less, &[&a, &b])? {
                let result = self.call_function(&f.function, f.module, None, vec![a, b], false, "operator<")?;
                return result.as_bool().ok_or_else(|| error("operator< must return a boolean"));
            }
            if let (Some(ta), Some(tb)) = (a.tag, b.tag)
                && types::is_enum(ta)
            {
                if ta != tb {
                    return Err(error("Cannot compare values of different enums"));
                }
                let index = |v: &Value| v.as_str().and_then(|s| types::enum_index(ta, s));
                return Ok(index(&a) < index(&b));
            }
        }
        match (&a.kind, &b.kind) {
            (Kind::Number(x), Kind::Number(y)) => Ok(x < y),
            (Kind::String(x), Kind::String(y)) => Ok(x.chars().lt(y.chars())),
            _ => Err(error(format!("< can't be applied to {} and {}", a.type_description(), b.type_description()))),
        }
    }

    fn negate(&mut self, value: Value) -> R<Value> {
        if value.tag.is_some()
            && let Some(candidates) = self.modules.operators.get(&OverloadableOp::Sub).cloned()
        {
            let unary: Vec<FunctionRef> = candidates.into_iter().filter(|f| f.function.params.len() == 1).collect();
            if let Ok(f) = self.resolve_overload("operator-", &unary, std::slice::from_ref(&value)) {
                return self.call_function(&f.function, f.module, None, vec![value], false, "operator-");
            }
        }
        match value.as_number() {
            Some(n) => Ok(Value::number(-n)),
            None => Err(error(format!("Unary - can't be applied to {}", value.type_description()))),
        }
    }

    fn condition(&mut self, value: Value) -> R<bool> {
        value.as_bool().ok_or_else(|| {
            error(format!(
                "First operand of logical or conditional operator must be boolean, value is {}",
                value.type_description()
            ))
        })
    }

    // ----- Expressions -----

    pub fn eval(&mut self, expr: &Expr) -> R<Value> {
        Ok(match &expr.kind {
            ExprKind::Number(n) => Value::number(*n),
            ExprKind::String(s) => Value::string(s.as_str()),
            ExprKind::Bool(b) => Value::bool(*b),
            ExprKind::Undefined => Value::UNDEFINED,
            ExprKind::Inf => Value::number(f64::INFINITY),
            ExprKind::Name(path) => self.lookup_name(path)?,
            ExprKind::Builtin(name) => return Err(error(format!("@{name} can only be called"))),
            ExprKind::Array(items) => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(self.eval(item)?);
                }
                Value::array(values)
            }
            ExprKind::Map(entries) => {
                let mut map = ValueMap::new();
                for entry in entries {
                    let key = match &entry.key {
                        MapKey::Name(name) => Value::string(name.name.as_str()),
                        MapKey::Expr(key) => self.eval(key)?,
                    };
                    let value = self.eval(&entry.value)?;
                    // A map never holds undefined.
                    if !value.is_undefined() {
                        map.insert(key, value);
                    }
                }
                Value::map(map)
            }
            ExprKind::Unary { op, operand } => {
                let value = self.eval(operand)?;
                match op {
                    UnaryOp::Neg => self.negate(value)?,
                    UnaryOp::Not => match value.as_bool() {
                        Some(b) => Value::bool(!b),
                        None => return Err(error(format!("! can't be applied to {}", value.type_description()))),
                    },
                }
            }
            ExprKind::Binary { op, lhs, rhs } => self.binary(*op, lhs, rhs)?,
            ExprKind::Ternary { cond, then, otherwise } => {
                let c = self.eval(cond)?;
                if self.condition(c)? { self.eval(then)? } else { self.eval(otherwise)? }
            }
            ExprKind::Is { operand, ty } => {
                let value = self.eval(operand)?;
                let test = self.resolve_type(ty)?;
                Value::bool(test.matches(&value))
            }
            ExprKind::As { operand, ty } => {
                let value = self.eval(operand)?;
                self.cast(value, ty)?
            }
            ExprKind::Call { callee, args } => match &callee.kind {
                ExprKind::Builtin(name) => {
                    let args = self.eval_args(args)?;
                    self.call_builtin(name, args)?
                }
                ExprKind::Name(path) => {
                    let args = self.eval_args(args)?;
                    self.call_by_name(path, args)?
                }
                _ => {
                    let f = self.eval(callee)?;
                    let args = self.eval_args(args)?;
                    self.call_value(&f, args)?
                }
            },
            ExprKind::Pipe { receiver, function, args } => {
                let mut values = vec![self.eval(receiver)?];
                values.extend(self.eval_args(args)?);
                self.call_by_name(function, values)?
            }
            ExprKind::Index { target, index } => {
                let target = self.eval(target)?;
                let index = self.eval(index)?;
                index_value(&target, &index)?
            }
            ExprKind::Member { target, name, optional } => {
                let target = self.eval(target)?;
                if *optional && target.is_undefined() {
                    Value::UNDEFINED
                } else {
                    index_value(&target, &Value::string(name.name.as_str()))?
                }
            }
            ExprKind::BoxValue(target) => {
                let target = self.eval(target)?;
                match &target.kind {
                    Kind::Box(cell) => cell.value.borrow().clone(),
                    _ => return Err(error(format!("[] needs a box, not {}", target.type_description()))),
                }
            }
            ExprKind::NewBox(value) => Value::new_box(self.eval(value)?),
            ExprKind::Lambda(function) => {
                let captured = self.capture();
                Value::function(FunctionKind::Lambda { function: function.clone(), module: self.module, captured })
            }
            ExprKind::Switch { value, cases } => {
                let value = self.eval(value)?;
                for (key, result) in cases {
                    if self.eval(key)? == value {
                        return self.eval(result);
                    }
                }
                Value::UNDEFINED
            }
            ExprKind::Try { expr, .. } => match self.eval(expr) {
                Ok(v) => v,
                Err(Flow::Throw(exception)) => {
                    if self.trace_catches {
                        eprintln!("caught by try(): {exception:?}");
                    }
                    Value::UNDEFINED
                }
                Err(other) => return Err(other),
            },
        })
    }

    fn eval_args(&mut self, args: &[Expr]) -> R<Vec<Value>> {
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.eval(arg)?);
        }
        Ok(values)
    }

    /// The variables a lambda created here can see, by value.
    fn capture(&self) -> Captured {
        let mut captured: Vec<(Rc<str>, Value)> = match &self.captured {
            Some(outer) => outer.as_ref().clone(),
            None => Vec::new(),
        };
        for local in &self.locals[self.frame_base..] {
            captured.push((local.name.clone(), local.value.clone()));
        }
        Rc::new(captured)
    }

    fn binary(&mut self, op: BinaryOp, lhs: &Expr, rhs: &Expr) -> R<Value> {
        match op {
            BinaryOp::And | BinaryOp::Or => {
                let a = self.eval(lhs)?;
                let a = self.condition(a)?;
                if (op == BinaryOp::And) != a {
                    return Ok(Value::bool(a));
                }
                let b = self.eval(rhs)?;
                Ok(Value::bool(self.condition(b)?))
            }
            BinaryOp::Coalesce => {
                let a = self.eval(lhs)?;
                if a.is_undefined() { self.eval(rhs) } else { Ok(a) }
            }
            _ => {
                let a = self.eval(lhs)?;
                let b = self.eval(rhs)?;
                self.binary_values(op, a, b)
            }
        }
    }

    pub fn binary_values(&mut self, op: BinaryOp, a: Value, b: Value) -> R<Value> {
        Ok(match op {
            BinaryOp::Add => self.arithmetic(OverloadableOp::Add, a, b)?,
            BinaryOp::Sub => self.arithmetic(OverloadableOp::Sub, a, b)?,
            BinaryOp::Mul => self.arithmetic(OverloadableOp::Mul, a, b)?,
            BinaryOp::Div => self.arithmetic(OverloadableOp::Div, a, b)?,
            BinaryOp::Rem => self.arithmetic(OverloadableOp::Rem, a, b)?,
            BinaryOp::Pow => self.arithmetic(OverloadableOp::Pow, a, b)?,
            BinaryOp::Concat => Value::string(concat_text(&a) + &concat_text(&b)),
            BinaryOp::Equal => Value::bool(a == b),
            BinaryOp::NotEqual => Value::bool(a != b),
            BinaryOp::Less => Value::bool(self.less(a, b)?),
            BinaryOp::Greater => Value::bool(self.less(b, a)?),
            BinaryOp::LessEqual => Value::bool(!self.less(b, a)?),
            BinaryOp::GreaterEqual => Value::bool(!self.less(a, b)?),
            BinaryOp::And | BinaryOp::Or => Value::bool(self.condition(a)? && self.condition(b)?),
            BinaryOp::Coalesce => {
                if a.is_undefined() {
                    b
                } else {
                    a
                }
            }
        })
    }

    // ----- Statements -----

    pub fn exec_block(&mut self, block: &ast::Block) -> R<()> {
        self.in_scope(|this| {
            for stmt in &block.stmts {
                this.exec_stmt(stmt)?;
            }
            Ok(())
        })
    }

    pub fn exec_stmt(&mut self, stmt: &Stmt) -> R<()> {
        match &stmt.kind {
            StmtKind::Decl { kind, name, ty, init } => {
                let value = match init {
                    Some(init) => self.eval(init)?,
                    None => Value::UNDEFINED,
                };
                if let Some(ty) = ty {
                    let test = self.resolve_type(ty)?;
                    if !test.matches(&value) {
                        return Err(error(format!(
                            "{} is {}, not {}",
                            name.name,
                            value.type_description(),
                            ty.path.name.name
                        )));
                    }
                }
                self.declare(&name.name, value, *kind == DeclKind::Const);
            }
            StmtKind::Assign { target, op, value } => self.assign(target, *op, value)?,
            StmtKind::Expr(expr) => {
                self.eval(expr)?;
            }
            StmtKind::Block(block) => self.exec_block(block)?,
            StmtKind::If { cond, then, otherwise } => {
                let c = self.eval(cond)?;
                if self.condition(c)? {
                    self.in_scope(|this| this.exec_stmt(then))?;
                } else if let Some(otherwise) = otherwise {
                    self.in_scope(|this| this.exec_stmt(otherwise))?;
                }
            }
            StmtKind::While { cond, body } => loop {
                let c = self.eval(cond)?;
                if !self.condition(c)? {
                    break;
                }
                match self.in_scope(|this| this.exec_stmt(body)) {
                    Ok(()) | Err(Flow::Continue) => {}
                    Err(Flow::Break) => break,
                    Err(e) => return Err(e),
                }
            },
            StmtKind::For { init, cond, step, body } => self.in_scope(|this| {
                if let Some(init) = init {
                    this.exec_stmt(init)?;
                }
                loop {
                    if let Some(cond) = cond {
                        let c = this.eval(cond)?;
                        if !this.condition(c)? {
                            break;
                        }
                    }
                    match this.in_scope(|this| this.exec_stmt(body)) {
                        Ok(()) | Err(Flow::Continue) => {}
                        Err(Flow::Break) => break,
                        Err(e) => return Err(e),
                    }
                    if let Some(step) = step {
                        this.exec_stmt(step)?;
                    }
                }
                Ok(())
            })?,
            StmtKind::ForIn { decl, key, value, iterable, body } => {
                let iterable = self.eval(iterable)?;
                self.for_each(&iterable, *decl, key.as_ref(), value, |this| match this.exec_stmt(body) {
                    Ok(()) | Err(Flow::Continue) => Ok(true),
                    Err(Flow::Break) => Ok(false),
                    Err(e) => Err(e),
                })?;
            }
            StmtKind::Return(value) => {
                let value = match value {
                    Some(v) => self.eval(v)?,
                    None => Value::UNDEFINED,
                };
                return Err(Flow::Return(value));
            }
            StmtKind::Break => return Err(Flow::Break),
            StmtKind::Continue => return Err(Flow::Continue),
            StmtKind::Throw(value) => return Err(Flow::Throw(self.eval(value)?)),
            StmtKind::Try { body, catch, .. } => match self.exec_block(body) {
                Err(Flow::Throw(exception)) => {
                    if self.trace_catches {
                        eprintln!("caught: {exception:?}");
                    }
                    if let Some(catch) = catch {
                        self.in_scope(|this| {
                            if let Some(name) = &catch.name {
                                this.declare(&name.name, exception, false);
                            }
                            this.exec_block(&catch.body)
                        })?;
                    }
                }
                other => other?,
            },
        }
        Ok(())
    }

    /// Runs `body` once per element of an array or map, binding the loop variables. `body` returns false to stop.
    fn for_each(
        &mut self,
        iterable: &Value,
        decl: Option<DeclKind>,
        key: Option<&ast::Ident>,
        value: &ast::Ident,
        mut body: impl FnMut(&mut Self) -> R<bool>,
    ) -> R<()> {
        let pairs: Vec<(Value, Value)> = match &iterable.kind {
            Kind::Array(items) => items.iter().enumerate().map(|(i, v)| (Value::number(i as f64), v.clone())).collect(),
            Kind::Map(entries) => entries.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            _ => return Err(error(format!("Can't iterate over {}", iterable.type_description()))),
        };
        let is_map = matches!(iterable.kind, Kind::Map(_));
        for (k, v) in pairs {
            let element = if key.is_none() && is_map {
                let mut entry = ValueMap::new();
                entry.insert(Value::string("key"), k.clone());
                entry.insert(Value::string("value"), v.clone());
                Value::map(entry)
            } else {
                v
            };
            let keep_going = self.in_scope(|this| {
                let bindings: Vec<(&ast::Ident, Value)> = match key {
                    Some(key) => vec![(key, k), (value, element)],
                    None => vec![(value, element)],
                };
                for (ident, v) in bindings {
                    match decl {
                        Some(kind) => this.declare(&ident.name, v, kind == DeclKind::Const),
                        None => this.store_variable(&ident.name, v)?,
                    }
                }
                body(this)
            })?;
            if !keep_going {
                break;
            }
        }
        Ok(())
    }

    // ----- Predicates -----

    pub fn exec_predicate_block(&mut self, block: &ast::Block) -> R<bool> {
        self.in_scope(|this| {
            for stmt in &block.stmts {
                if !this.exec_predicate(stmt)? {
                    return Ok(false);
                }
            }
            Ok(true)
        })
    }

    /// Runs a statement in predicate mode: an expression statement must be true for the predicate to hold.
    pub fn exec_predicate(&mut self, stmt: &Stmt) -> R<bool> {
        match &stmt.kind {
            StmtKind::Expr(expr) => {
                let value = self.eval(expr)?;
                let holds = value.as_bool().ok_or_else(|| {
                    error(format!("Predicate statement must be boolean, value is {}", value.type_description()))
                })?;
                if !holds && self.failed_predicate.is_none() {
                    self.failed_predicate = Some(fs_lang::printer::print_expr(expr));
                }
                Ok(holds)
            }
            StmtKind::Block(block) => self.exec_predicate_block(block),
            StmtKind::If { cond, then, otherwise } => {
                // A predicate found false in the condition just picks the branch: it isn't why anything failed.
                let before = self.failed_predicate.clone();
                let c = self.eval(cond)?;
                self.failed_predicate = before;
                if self.condition(c)? {
                    self.in_scope(|this| this.exec_predicate(then))
                } else if let Some(otherwise) = otherwise {
                    self.in_scope(|this| this.exec_predicate(otherwise))
                } else {
                    Ok(true)
                }
            }
            // Loops check their bodies as predicate statements; `continue` and `break` work as in any loop.
            StmtKind::ForIn { decl, key, value, iterable, body } => {
                let iterable = self.eval(iterable)?;
                let mut holds = true;
                self.for_each(&iterable, *decl, key.as_ref(), value, |this| match this.exec_predicate(body) {
                    Ok(h) => {
                        holds = h;
                        Ok(h)
                    }
                    Err(Flow::Continue) => Ok(true),
                    Err(Flow::Break) => Ok(false),
                    Err(e) => Err(e),
                })?;
                Ok(holds)
            }
            StmtKind::For { init, cond, step, body } => self.in_scope(|this| {
                if let Some(init) = init {
                    this.exec_stmt(init)?;
                }
                loop {
                    if let Some(cond) = cond {
                        let c = this.eval(cond)?;
                        if !this.condition(c)? {
                            return Ok(true);
                        }
                    }
                    match this.in_scope(|this| this.exec_predicate(body)) {
                        Ok(true) | Err(Flow::Continue) => {}
                        Ok(false) => return Ok(false),
                        Err(Flow::Break) => return Ok(true),
                        Err(e) => return Err(e),
                    }
                    if let Some(step) = step {
                        this.exec_stmt(step)?;
                    }
                }
            }),
            StmtKind::While { cond, body } => loop {
                let c = self.eval(cond)?;
                if !self.condition(c)? {
                    return Ok(true);
                }
                match self.in_scope(|this| this.exec_predicate(body)) {
                    Ok(true) | Err(Flow::Continue) => {}
                    Ok(false) => return Ok(false),
                    Err(Flow::Break) => return Ok(true),
                    Err(e) => return Err(e),
                }
            },
            StmtKind::Return(Some(value)) => {
                let value = self.eval(value)?;
                value.as_bool().ok_or_else(|| error("A predicate must return a boolean"))
            }
            _ => {
                self.exec_stmt(stmt)?;
                Ok(true)
            }
        }
    }

    // ----- Assignment -----

    fn assign(&mut self, target: &Expr, op: AssignOp, value: &Expr) -> R<()> {
        let new_value = if op == AssignOp::Assign {
            self.eval(value)?
        } else {
            let old = self.eval(target)?;
            match op {
                AssignOp::And | AssignOp::Or => {
                    let a = self.condition(old)?;
                    if (op == AssignOp::And) != a {
                        Value::bool(a)
                    } else {
                        let b = self.eval(value)?;
                        Value::bool(self.condition(b)?)
                    }
                }
                AssignOp::Coalesce => {
                    if old.is_undefined() {
                        self.eval(value)?
                    } else {
                        old
                    }
                }
                _ => {
                    let rhs = self.eval(value)?;
                    let binary = match op {
                        AssignOp::Add => BinaryOp::Add,
                        AssignOp::Sub => BinaryOp::Sub,
                        AssignOp::Mul => BinaryOp::Mul,
                        AssignOp::Div => BinaryOp::Div,
                        AssignOp::Rem => BinaryOp::Rem,
                        AssignOp::Pow => BinaryOp::Pow,
                        AssignOp::Concat => BinaryOp::Concat,
                        _ => unreachable!(),
                    };
                    self.binary_values(binary, old, rhs)?
                }
            }
        };
        self.store(target, new_value)
    }

    /// Stores `value` at an assignment target such as `a`, `a.b[c]` or `f(x)[]`.
    fn store(&mut self, target: &Expr, value: Value) -> R<()> {
        // Collect the access path from the root outwards.
        let mut steps_rev = Vec::new();
        let mut node = target;
        loop {
            match &node.kind {
                ExprKind::Member { target, name, .. } => {
                    steps_rev.push(Step::Key(Value::string(name.name.as_str())));
                    node = target;
                }
                ExprKind::Index { target, index } => {
                    steps_rev.push(Step::Key(self.eval(index)?));
                    node = target;
                }
                ExprKind::BoxValue(target) => {
                    // Everything before a box is only read: the box is a reference.
                    let holder = self.eval(target)?;
                    let Kind::Box(cell) = &holder.kind else {
                        return Err(error(format!("[] needs a box, not {}", holder.type_description())));
                    };
                    steps_rev.reverse();
                    let mut inner = cell.value.borrow_mut();
                    return apply_steps(&mut inner, &steps_rev, value);
                }
                ExprKind::Name(path) if path.namespace.is_none() => {
                    steps_rev.reverse();
                    return self.store_path(&path.name.name, &steps_rev, value);
                }
                _ => return Err(error("Can't assign to this expression")),
            }
        }
    }

    fn store_variable(&mut self, name: &str, value: Value) -> R<()> {
        self.store_path(name, &[], value)
    }

    fn store_path(&mut self, name: &str, steps: &[Step], value: Value) -> R<()> {
        let base = self.frame_base;
        for local in self.locals[base..].iter_mut().rev() {
            if &*local.name == name {
                if local.constant {
                    return Err(error(format!("Can't assign to constant {name}")));
                }
                return apply_steps(&mut local.value, steps, value);
            }
        }
        Err(error(format!("Can't assign to {name}: not a variable in this function")))
    }
}

fn apply_steps(slot: &mut Value, steps: &[Step], value: Value) -> R<()> {
    let Some((first, rest)) = steps.split_first() else {
        *slot = value;
        return Ok(());
    };
    match first {
        Step::Key(key) => match &mut slot.kind {
            Kind::Map(map) => {
                let map = Rc::make_mut(map);
                if rest.is_empty() {
                    if value.is_undefined() {
                        map.remove(key);
                    } else {
                        map.insert(key.clone(), value);
                    }
                    Ok(())
                } else {
                    match map.get_mut(key) {
                        Some(child) => apply_steps(child, rest, value),
                        None => Err(error(format!("Can't assign inside missing key {key:?}"))),
                    }
                }
            }
            Kind::Array(items) => {
                let index = array_index(key, items.len())?;
                let items = Rc::make_mut(items);
                apply_steps(&mut items[index], rest, value)
            }
            _ => Err(error(format!("Can't assign into {}", slot.type_description()))),
        },
    }
}

fn array_index(key: &Value, len: usize) -> R<usize> {
    let Some(n) = key.as_number() else {
        return Err(error(format!("Array index must be a number, not {}", key.type_description())));
    };
    if n < 0.0 || n.fract() != 0.0 || n as usize >= len {
        return Err(error(format!("Array index {n} out of bounds (size {len})")));
    }
    Ok(n as usize)
}

pub fn index_value(target: &Value, key: &Value) -> R<Value> {
    match &target.kind {
        Kind::Map(map) => Ok(map.get(key).cloned().unwrap_or(Value::UNDEFINED)),
        Kind::Array(items) => Ok(items[array_index(key, items.len())?].clone()),
        Kind::Undefined => Err(error(format!("Can't read {key:?} from undefined"))),
        _ => Err(error(format!("Can't read {key:?} from {}", target.type_description()))),
    }
}

fn operator_symbol(op: OverloadableOp) -> &'static str {
    match op {
        OverloadableOp::Add => "+",
        OverloadableOp::Sub => "-",
        OverloadableOp::Mul => "*",
        OverloadableOp::Div => "/",
        OverloadableOp::Rem => "%",
        OverloadableOp::Pow => "^",
        OverloadableOp::Less => "<",
    }
}

// ----- Entry points -----

impl Interp {
    /// Evaluates an expression as if it appeared at the top level of `module`.
    pub fn eval_in_module(&mut self, module: ModuleId, expr: &Expr) -> R<Value> {
        self.in_frame(module, None, |this| this.eval(expr))
    }

    /// Calls the function `name` as seen from `module`.
    pub fn call_in_module(&mut self, module: ModuleId, name: &str, args: Vec<Value>) -> R<Value> {
        let path = ast::Path {
            namespace: None,
            name: ast::Ident { name: name.to_string(), span: Default::default() },
            span: Default::default(),
        };
        self.in_frame(module, None, |this| this.call_by_name(&path, args))
    }
}
