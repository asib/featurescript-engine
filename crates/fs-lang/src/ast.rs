//! The FeatureScript syntax tree.

use crate::span::Span;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

/// A name, optionally qualified by an import namespace: `foo` or `ns::foo`.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub namespace: Option<Ident>,
    pub name: Ident,
    pub span: Span,
}

/// A type in `x is T`, `x as T`, a parameter constraint or a return type.
///
/// Most are names (`number`, `Vector`, `ns::Query`). `undefined` and `function` are keywords but also name types.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeName {
    pub path: Path,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    /// The number in the `FeatureScript 3112;` header.
    pub version: Option<u32>,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// `annotation { ... }` maps written before the item.
    pub annotations: Vec<Expr>,
    pub exported: bool,
    pub kind: ItemKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ItemKind {
    /// `import(path : "...", version : "...");`, optionally `ns::import(...)`.
    Import {
        namespace: Option<Ident>,
        path: String,
        version: String,
    },
    Const {
        name: Ident,
        ty: Option<TypeName>,
        value: Expr,
    },
    Function {
        name: Ident,
        function: Rc<Function>,
    },
    /// A function whose body runs in predicate mode: each expression statement must be true.
    Predicate {
        name: Ident,
        function: Rc<Function>,
    },
    /// `operator+(a is T, b is U) { ... }`.
    Operator {
        op: OverloadableOp,
        function: Rc<Function>,
    },
    /// `type T typecheck canBeT;`
    Type {
        name: Ident,
        typecheck: Path,
    },
    Enum {
        name: Ident,
        values: Vec<EnumValue>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverloadableOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    Less,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumValue {
    pub annotations: Vec<Expr>,
    pub name: Ident,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub name: Ident,
    pub ty: Option<TypeName>,
}

/// Parameters, return type, precondition and body, shared by named functions, operators and lambdas.
#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub params: Vec<Param>,
    pub returns: Option<TypeName>,
    pub precondition: Option<Box<Stmt>>,
    pub body: FunctionBody,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FunctionBody {
    Block(Block),
    /// The expression after `=>` in a short lambda.
    Expr(Box<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stmt {
    /// `annotation { ... }` maps written before the statement, as on feature parameters in a precondition.
    pub annotations: Vec<Expr>,
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclKind {
    Var,
    Const,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StmtKind {
    Decl {
        kind: DeclKind,
        name: Ident,
        ty: Option<TypeName>,
        init: Option<Expr>,
    },
    Assign {
        target: Expr,
        op: AssignOp,
        value: Expr,
    },
    Expr(Expr),
    Block(Block),
    If {
        cond: Expr,
        then: Box<Stmt>,
        otherwise: Option<Box<Stmt>>,
    },
    While {
        cond: Expr,
        body: Box<Stmt>,
    },
    For {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        step: Option<Box<Stmt>>,
        body: Box<Stmt>,
    },
    /// `for (var k, v in x)`; `decl` is `None` when the loop reuses existing variables.
    ForIn {
        decl: Option<DeclKind>,
        key: Option<Ident>,
        value: Ident,
        iterable: Expr,
        body: Box<Stmt>,
    },
    Return(Option<Expr>),
    Break,
    Continue,
    Throw(Expr),
    /// `try { } catch (e) { }` or `try silent { }`.
    Try {
        silent: bool,
        body: Block,
        catch: Option<Catch>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Catch {
    pub name: Option<Ident>,
    pub body: Block,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    Concat,
    And,
    Or,
    Coalesce,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Number(f64),
    String(String),
    Bool(bool),
    Undefined,
    Inf,
    Name(Path),
    /// `@name`, a function provided by the runtime.
    Builtin(String),
    Array(Vec<Expr>),
    Map(Vec<MapEntry>),
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Ternary {
        cond: Box<Expr>,
        then: Box<Expr>,
        otherwise: Box<Expr>,
    },
    Is {
        operand: Box<Expr>,
        ty: TypeName,
    },
    As {
        operand: Box<Expr>,
        ty: TypeName,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    /// `receiver->f(args)`, which means `f(receiver, args)`.
    Pipe {
        receiver: Box<Expr>,
        function: Path,
        args: Vec<Expr>,
    },
    Index {
        target: Box<Expr>,
        index: Box<Expr>,
    },
    /// `x.name`, or `x?.name` when `optional`.
    Member {
        target: Box<Expr>,
        name: Ident,
        optional: bool,
    },
    /// `x[]`: the value in a box.
    BoxValue(Box<Expr>),
    /// `new box(value)`.
    NewBox(Box<Expr>),
    Lambda(Rc<Function>),
    /// `switch (value) { key : result, ... }`.
    Switch {
        value: Box<Expr>,
        cases: Vec<(Expr, Expr)>,
    },
    /// `try(expr)` or `try silent(expr)`: `undefined` if `expr` throws.
    Try {
        silent: bool,
        expr: Box<Expr>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapEntry {
    pub key: MapKey,
    pub value: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MapKey {
    /// A bare name, which stands for the string with that text: `{ a : 1 }`.
    Name(Ident),
    /// Any other key expression: a string literal, `(expr)`, an enum value, and so on.
    Expr(Expr),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    Concat,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    Equal,
    NotEqual,
    And,
    Or,
    Coalesce,
}
