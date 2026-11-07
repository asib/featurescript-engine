//! Prints a syntax tree back as FeatureScript source.
//!
//! The output parses back to the same tree, apart from spans. It adds parentheses only where precedence needs
//! them, and uses std's brace style. It drops comments, which the syntax tree doesn't keep.

use crate::ast::*;
use crate::parser::{
    POSTFIX_BINDING_POWER, TERNARY_BINDING_POWER, TYPE_TEST_BINDING_POWER, UNARY_BINDING_POWER, binary_binding_power,
};
use std::fmt::Write as _;

pub fn print_module(module: &Module) -> String {
    let mut printer = Printer::default();
    printer.module(module);
    printer.out
}

pub fn print_expr(expr: &Expr) -> String {
    let mut printer = Printer::default();
    printer.expr(expr, 0);
    printer.out
}

/// Binding power at which an expression can appear without parentheses.
const PRIMARY: u8 = 100;

fn precedence(expr: &Expr) -> u8 {
    match &expr.kind {
        ExprKind::Lambda(_) => 0,
        ExprKind::Ternary { .. } => TERNARY_BINDING_POWER,
        ExprKind::Binary { op, .. } => binary_binding_power(*op).0,
        ExprKind::Is { .. } | ExprKind::As { .. } => TYPE_TEST_BINDING_POWER,
        ExprKind::Unary { .. } => UNARY_BINDING_POWER,
        ExprKind::Call { .. }
        | ExprKind::Pipe { .. }
        | ExprKind::Index { .. }
        | ExprKind::Member { .. }
        | ExprKind::BoxValue(_) => POSTFIX_BINDING_POWER,
        _ => PRIMARY,
    }
}

const MAP_KEY_BINDING_POWER: u8 = TERNARY_BINDING_POWER + 1;

#[derive(Default)]
struct Printer {
    out: String,
    indent: usize,
}

impl Printer {
    fn line(&mut self) {
        self.out.push('\n');
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
    }

    fn module(&mut self, module: &Module) {
        if let Some(version) = module.version {
            let _ = writeln!(self.out, "FeatureScript {version};");
        }
        for item in &module.items {
            self.out.push('\n');
            self.item(item);
            self.out.push('\n');
        }
    }

    fn annotations(&mut self, annotations: &[Expr]) {
        for annotation in annotations {
            self.out.push_str("annotation ");
            self.expr(annotation, 0);
            self.line();
        }
    }

    fn item(&mut self, item: &Item) {
        self.annotations(&item.annotations);
        if item.exported {
            self.out.push_str("export ");
        }
        match &item.kind {
            ItemKind::Import { namespace, path, version } => {
                if let Some(namespace) = namespace {
                    let _ = write!(self.out, "{}::", namespace.name);
                }
                self.out.push_str("import(path : ");
                self.string(path);
                self.out.push_str(", version : ");
                self.string(version);
                self.out.push_str(");");
            }
            ItemKind::Const { name, ty, value } => {
                let _ = write!(self.out, "const {}", name.name);
                if let Some(ty) = ty {
                    self.out.push_str(" is ");
                    self.type_name(ty);
                }
                self.out.push_str(" = ");
                self.expr(value, 0);
                self.out.push(';');
            }
            ItemKind::Function { name, function } => {
                let _ = write!(self.out, "function {}", name.name);
                self.function(function);
            }
            ItemKind::Predicate { name, params, precondition, body } => {
                let _ = write!(self.out, "predicate {}", name.name);
                self.params(params);
                if let Some(precondition) = precondition {
                    self.line();
                    self.out.push_str("precondition");
                    self.indented_stmt(precondition);
                }
                self.line();
                self.block(body);
            }
            ItemKind::Operator { op, function } => {
                let symbol = match op {
                    OverloadableOp::Add => "+",
                    OverloadableOp::Sub => "-",
                    OverloadableOp::Mul => "*",
                    OverloadableOp::Div => "/",
                    OverloadableOp::Rem => "%",
                    OverloadableOp::Pow => "^",
                    OverloadableOp::Less => "<",
                };
                let _ = write!(self.out, "operator{symbol}");
                self.function(function);
            }
            ItemKind::Type { name, typecheck } => {
                let _ = write!(self.out, "type {} typecheck ", name.name);
                self.path(typecheck);
                self.out.push(';');
            }
            ItemKind::Enum { name, values } => {
                let _ = write!(self.out, "enum {}", name.name);
                self.line();
                self.out.push('{');
                self.indent += 1;
                for (i, value) in values.iter().enumerate() {
                    self.line();
                    self.annotations(&value.annotations);
                    self.out.push_str(&value.name.name);
                    if i + 1 < values.len() {
                        self.out.push(',');
                    }
                }
                self.indent -= 1;
                self.line();
                self.out.push('}');
            }
        }
    }

    fn params(&mut self, params: &[Param]) {
        self.out.push('(');
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                self.out.push_str(", ");
            }
            self.out.push_str(&param.name.name);
            if let Some(ty) = &param.ty {
                self.out.push_str(" is ");
                self.type_name(ty);
            }
        }
        self.out.push(')');
    }

    /// Everything after the name: parameters, return type, precondition and body.
    fn function(&mut self, function: &Function) {
        self.params(&function.params);
        if let Some(returns) = &function.returns {
            self.out.push_str(" returns ");
            self.type_name(returns);
        }
        if let Some(precondition) = &function.precondition {
            self.line();
            self.out.push_str("precondition");
            self.indented_stmt(precondition);
        }
        match &function.body {
            FunctionBody::Block(block) => {
                self.line();
                self.block(block);
            }
            FunctionBody::Expr(_) => unreachable!("named functions and `function` lambdas have block bodies"),
        }
    }

    fn path(&mut self, path: &Path) {
        if let Some(namespace) = &path.namespace {
            let _ = write!(self.out, "{}::", namespace.name);
        }
        self.out.push_str(&path.name.name);
    }

    fn type_name(&mut self, ty: &TypeName) {
        self.path(&ty.path);
    }

    fn string(&mut self, value: &str) {
        self.out.push('"');
        for c in value.chars() {
            match c {
                '"' => self.out.push_str("\\\""),
                '\\' => self.out.push_str("\\\\"),
                '\n' => self.out.push_str("\\n"),
                '\t' => self.out.push_str("\\t"),
                '\r' => self.out.push_str("\\r"),
                '\u{8}' => self.out.push_str("\\b"),
                '\u{c}' => self.out.push_str("\\f"),
                c if (c as u32) < 0x20 => {
                    let _ = write!(self.out, "\\u{:04x}", c as u32);
                }
                c => self.out.push(c),
            }
        }
        self.out.push('"');
    }

    // ----- Statements -----

    fn block(&mut self, block: &Block) {
        self.out.push('{');
        self.indent += 1;
        for stmt in &block.stmts {
            self.line();
            self.stmt(stmt);
        }
        self.indent -= 1;
        self.line();
        self.out.push('}');
    }

    /// A statement that follows a header such as `if (...)`: a block on its own line, or an indented statement.
    fn indented_stmt(&mut self, stmt: &Stmt) {
        if matches!(stmt.kind, StmtKind::Block(_)) && stmt.annotations.is_empty() {
            self.line();
            self.stmt(stmt);
        } else {
            self.indent += 1;
            self.line();
            self.stmt(stmt);
            self.indent -= 1;
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        self.annotations(&stmt.annotations);
        match &stmt.kind {
            StmtKind::Block(block) => self.block(block),
            StmtKind::If { cond, then, otherwise } => {
                self.out.push_str("if (");
                self.expr(cond, 0);
                self.out.push(')');
                self.indented_stmt(then);
                if let Some(otherwise) = otherwise {
                    self.line();
                    self.out.push_str("else");
                    if matches!(otherwise.kind, StmtKind::If { .. }) && otherwise.annotations.is_empty() {
                        self.out.push(' ');
                        self.stmt(otherwise);
                    } else {
                        self.indented_stmt(otherwise);
                    }
                }
            }
            StmtKind::While { cond, body } => {
                self.out.push_str("while (");
                self.expr(cond, 0);
                self.out.push(')');
                self.indented_stmt(body);
            }
            StmtKind::For { init, cond, step, body } => {
                self.out.push_str("for (");
                if let Some(init) = init {
                    self.simple_stmt(init);
                }
                self.out.push(';');
                if let Some(cond) = cond {
                    self.out.push(' ');
                    self.expr(cond, 0);
                }
                self.out.push(';');
                if let Some(step) = step {
                    self.out.push(' ');
                    self.simple_stmt(step);
                }
                self.out.push(')');
                self.indented_stmt(body);
            }
            StmtKind::ForIn { decl, key, value, iterable, body } => {
                self.out.push_str("for (");
                match decl {
                    Some(DeclKind::Var) => self.out.push_str("var "),
                    Some(DeclKind::Const) => self.out.push_str("const "),
                    None => {}
                }
                if let Some(key) = key {
                    let _ = write!(self.out, "{}, ", key.name);
                }
                let _ = write!(self.out, "{} in ", value.name);
                self.expr(iterable, 0);
                self.out.push(')');
                self.indented_stmt(body);
            }
            StmtKind::Return(value) => {
                self.out.push_str("return");
                if let Some(value) = value {
                    self.out.push(' ');
                    self.expr(value, 0);
                }
                self.out.push(';');
            }
            StmtKind::Break => self.out.push_str("break;"),
            StmtKind::Continue => self.out.push_str("continue;"),
            StmtKind::Throw(value) => {
                self.out.push_str("throw ");
                self.expr(value, 0);
                self.out.push(';');
            }
            StmtKind::Try { silent, body, catch } => {
                self.out.push_str(if *silent { "try silent" } else { "try" });
                self.line();
                self.block(body);
                if let Some(catch) = catch {
                    self.line();
                    self.out.push_str("catch");
                    if let Some(name) = &catch.name {
                        let _ = write!(self.out, " ({})", name.name);
                    }
                    self.line();
                    self.block(&catch.body);
                }
            }
            StmtKind::Decl { .. } | StmtKind::Assign { .. } | StmtKind::Expr(_) => {
                self.simple_stmt(stmt);
                self.out.push(';');
            }
        }
    }

    fn simple_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Decl { kind, name, ty, init } => {
                self.out.push_str(match kind {
                    DeclKind::Var => "var ",
                    DeclKind::Const => "const ",
                });
                self.out.push_str(&name.name);
                if let Some(ty) = ty {
                    self.out.push_str(" is ");
                    self.type_name(ty);
                }
                if let Some(init) = init {
                    self.out.push_str(" = ");
                    self.expr(init, 0);
                }
            }
            StmtKind::Assign { target, op, value } => {
                self.expr(target, 0);
                self.out.push_str(match op {
                    AssignOp::Assign => " = ",
                    AssignOp::Add => " += ",
                    AssignOp::Sub => " -= ",
                    AssignOp::Mul => " *= ",
                    AssignOp::Div => " /= ",
                    AssignOp::Rem => " %= ",
                    AssignOp::Pow => " ^= ",
                    AssignOp::Concat => " ~= ",
                    AssignOp::And => " &&= ",
                    AssignOp::Or => " ||= ",
                    AssignOp::Coalesce => " ??= ",
                });
                self.expr(value, 0);
            }
            StmtKind::Expr(expr) => self.expr(expr, 0),
            _ => unreachable!("not a simple statement"),
        }
    }

    // ----- Expressions -----

    /// Prints `expr`, in parentheses if it binds more loosely than `min_bp`.
    fn expr(&mut self, expr: &Expr, min_bp: u8) {
        let parenthesise = precedence(expr) < min_bp;
        if parenthesise {
            self.out.push('(');
        }
        self.expr_inner(expr);
        if parenthesise {
            self.out.push(')');
        }
    }

    fn exprs(&mut self, exprs: &[Expr]) {
        for (i, expr) in exprs.iter().enumerate() {
            if i > 0 {
                self.out.push_str(", ");
            }
            self.expr(expr, 0);
        }
    }

    fn expr_inner(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Number(value) => {
                let _ = write!(self.out, "{value}");
            }
            ExprKind::String(value) => self.string(value),
            ExprKind::Bool(value) => self.out.push_str(if *value { "true" } else { "false" }),
            ExprKind::Undefined => self.out.push_str("undefined"),
            ExprKind::Inf => self.out.push_str("inf"),
            ExprKind::Name(path) => self.path(path),
            ExprKind::Builtin(name) => {
                let _ = write!(self.out, "@{name}");
            }
            ExprKind::Array(items) => {
                self.out.push('[');
                self.exprs(items);
                self.out.push(']');
            }
            ExprKind::Map(entries) => {
                if entries.is_empty() {
                    self.out.push_str("{}");
                    return;
                }
                self.out.push_str("{ ");
                for (i, entry) in entries.iter().enumerate() {
                    if i > 0 {
                        self.out.push_str(", ");
                    }
                    match &entry.key {
                        MapKey::Name(name) => self.out.push_str(&name.name),
                        // `{ (meter) : x }` keys on the value of `meter`; without parentheses it would be the
                        // string "meter".
                        MapKey::Expr(key @ Expr { kind: ExprKind::Name(Path { namespace: None, .. }), .. }) => {
                            self.out.push('(');
                            self.expr(key, 0);
                            self.out.push(')');
                        }
                        MapKey::Expr(key) => self.expr(key, MAP_KEY_BINDING_POWER),
                    }
                    self.out.push_str(" : ");
                    self.expr(&entry.value, 0);
                }
                self.out.push_str(" }");
            }
            ExprKind::Unary { op, operand } => {
                self.out.push(match op {
                    UnaryOp::Neg => '-',
                    UnaryOp::Not => '!',
                });
                // Keep `- -x` from printing as `--x`, which reads like a decrement.
                if matches!(operand.kind, ExprKind::Unary { .. }) {
                    self.out.push(' ');
                }
                self.expr(operand, UNARY_BINDING_POWER);
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let (left_bp, right_bp) = binary_binding_power(*op);
                let left_assoc = right_bp > left_bp;
                self.expr(lhs, if left_assoc { left_bp } else { left_bp + 1 });
                self.out.push_str(match op {
                    BinaryOp::Add => " + ",
                    BinaryOp::Sub => " - ",
                    BinaryOp::Mul => " * ",
                    BinaryOp::Div => " / ",
                    BinaryOp::Rem => " % ",
                    BinaryOp::Pow => "^",
                    BinaryOp::Concat => " ~ ",
                    BinaryOp::Less => " < ",
                    BinaryOp::Greater => " > ",
                    BinaryOp::LessEqual => " <= ",
                    BinaryOp::GreaterEqual => " >= ",
                    BinaryOp::Equal => " == ",
                    BinaryOp::NotEqual => " != ",
                    BinaryOp::And => " && ",
                    BinaryOp::Or => " || ",
                    BinaryOp::Coalesce => " ?? ",
                });
                self.expr(rhs, right_bp);
            }
            ExprKind::Ternary { cond, then, otherwise } => {
                self.expr(cond, TERNARY_BINDING_POWER + 1);
                self.out.push_str(" ? ");
                self.expr(then, 0);
                self.out.push_str(" : ");
                self.expr(otherwise, TERNARY_BINDING_POWER);
            }
            ExprKind::Is { operand, ty } | ExprKind::As { operand, ty } => {
                self.expr(operand, TYPE_TEST_BINDING_POWER);
                self.out.push_str(if matches!(expr.kind, ExprKind::Is { .. }) { " is " } else { " as " });
                self.type_name(ty);
            }
            ExprKind::Call { callee, args } => {
                self.expr(callee, POSTFIX_BINDING_POWER);
                self.out.push('(');
                self.exprs(args);
                self.out.push(')');
            }
            ExprKind::Pipe { receiver, function, args } => {
                self.expr(receiver, POSTFIX_BINDING_POWER);
                self.out.push_str("->");
                self.path(function);
                self.out.push('(');
                self.exprs(args);
                self.out.push(')');
            }
            ExprKind::Index { target, index } => {
                self.expr(target, POSTFIX_BINDING_POWER);
                self.out.push('[');
                self.expr(index, 0);
                self.out.push(']');
            }
            ExprKind::Member { target, name, optional } => {
                self.expr(target, POSTFIX_BINDING_POWER);
                self.out.push_str(if *optional { "?." } else { "." });
                self.out.push_str(&name.name);
            }
            ExprKind::BoxValue(target) => {
                self.expr(target, POSTFIX_BINDING_POWER);
                self.out.push_str("[]");
            }
            ExprKind::NewBox(value) => {
                self.out.push_str("new box(");
                self.expr(value, 0);
                self.out.push(')');
            }
            ExprKind::Lambda(function) => match &function.body {
                FunctionBody::Expr(body) => {
                    self.params(&function.params);
                    if let Some(returns) = &function.returns {
                        self.out.push_str(" returns ");
                        self.type_name(returns);
                    }
                    self.out.push_str(" => ");
                    // A map body needs parentheses; bare braces after `=>` would read as a block only if empty
                    // braces or a statement followed, but parentheses make the intent explicit either way.
                    if matches!(body.kind, ExprKind::Map(_)) {
                        self.out.push('(');
                        self.expr(body, 0);
                        self.out.push(')');
                    } else {
                        self.expr(body, 0);
                    }
                }
                FunctionBody::Block(_) => {
                    self.out.push_str("function");
                    self.function(function);
                }
            },
            ExprKind::Switch { value, cases } => {
                self.out.push_str("switch (");
                self.expr(value, 0);
                self.out.push_str(") { ");
                for (i, (key, result)) in cases.iter().enumerate() {
                    if i > 0 {
                        self.out.push_str(", ");
                    }
                    self.expr(key, MAP_KEY_BINDING_POWER);
                    self.out.push_str(" : ");
                    self.expr(result, 0);
                }
                self.out.push_str(" }");
            }
            ExprKind::Try { silent, expr } => {
                self.out.push_str(if *silent { "try silent(" } else { "try(" });
                self.expr(expr, 0);
                self.out.push(')');
            }
        }
    }
}
