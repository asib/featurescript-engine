//! Parses FeatureScript tokens into a syntax tree.
//!
//! Statements and top-level items use recursive descent; expressions use precedence climbing. Onshape doesn't
//! publish a precedence table, so the one below follows C-like conventions plus what std's own code implies. The
//! oracle should confirm it before the interpreter relies on the less common cases.
//!
//! Precedence, from loosest to tightest:
//!
//! | operators                              | associativity |
//! |----------------------------------------|---------------|
//! | `x => ...` lambdas                     | (prefix)      |
//! | `?:`                                   | right         |
//! | `??`                                   | left          |
//! | `\|\|`                                 | left          |
//! | `&&`                                   | left          |
//! | `==` `!=`                              | left          |
//! | `<` `>` `<=` `>=`                      | left          |
//! | `is T` `as T`                          | (postfix)     |
//! | `~`                                    | left          |
//! | `+` `-`                                | left          |
//! | `*` `/` `%`                            | left          |
//! | unary `-` `!`                          | (prefix)      |
//! | `^`                                    | right         |
//! | `()` `[]` `[i]` `.` `?.` `->`          | (postfix)     |
//!
//! `is` binds tighter than `==` because std writes `x is string == true`. `^` binds tighter than unary minus, so
//! `-x^2` is `-(x^2)`.

use crate::ast::*;
use crate::lexer::{self, Token, TokenKind, TokenKind as T};
use crate::span::{Span, line_col};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl ParseError {
    /// The message prefixed with `line:column`.
    pub fn describe(&self, source: &str) -> String {
        let (line, column) = line_col(source, self.span.start);
        format!("{line}:{column}: {}", self.message)
    }
}

type PResult<T> = Result<T, ParseError>;

pub fn parse_module(source: &str) -> PResult<Module> {
    Parser::new(source)?.module()
}

pub fn parse_expr(source: &str) -> PResult<Expr> {
    let mut parser = Parser::new(source)?;
    let expr = parser.expr()?;
    parser.expect(T::Eof)?;
    Ok(expr)
}

pub fn parse_stmt(source: &str) -> PResult<Stmt> {
    let mut parser = Parser::new(source)?;
    let stmt = parser.stmt()?;
    parser.expect(T::Eof)?;
    Ok(stmt)
}

// Binding powers.
const BP_TERNARY: u8 = 2;
const BP_TYPE_TEST: u8 = 13;
const BP_UNARY: u8 = 21;
const BP_POSTFIX: u8 = 30;
/// Map keys and switch cases are parsed above the ternary so their `:` isn't taken as part of one.
const BP_KEY: u8 = BP_TERNARY + 1;

fn binary_op(kind: TokenKind) -> Option<(BinaryOp, u8, u8)> {
    use BinaryOp::*;
    Some(match kind {
        T::QuestionQuestion => (Coalesce, 3, 4),
        T::OrOr => (Or, 5, 6),
        T::AndAnd => (And, 7, 8),
        T::EqualEqual => (Equal, 9, 10),
        T::BangEqual => (NotEqual, 9, 10),
        T::Less => (Less, 11, 12),
        T::Greater => (Greater, 11, 12),
        T::LessEqual => (LessEqual, 11, 12),
        T::GreaterEqual => (GreaterEqual, 11, 12),
        T::Tilde => (Concat, 15, 16),
        T::Plus => (Add, 17, 18),
        T::Minus => (Sub, 17, 18),
        T::Star => (Mul, 19, 20),
        T::Slash => (Div, 19, 20),
        T::Percent => (Rem, 19, 20),
        T::Caret => (Pow, 24, 24),
        _ => return None,
    })
}

/// The binding powers [`binary_op`] gives each operator, for the printer.
pub(crate) fn binary_binding_power(op: BinaryOp) -> (u8, u8) {
    use BinaryOp::*;
    match op {
        Coalesce => (3, 4),
        Or => (5, 6),
        And => (7, 8),
        Equal | NotEqual => (9, 10),
        Less | Greater | LessEqual | GreaterEqual => (11, 12),
        Concat => (15, 16),
        Add | Sub => (17, 18),
        Mul | Div | Rem => (19, 20),
        Pow => (24, 24),
    }
}

pub(crate) const TERNARY_BINDING_POWER: u8 = BP_TERNARY;
pub(crate) const TYPE_TEST_BINDING_POWER: u8 = BP_TYPE_TEST;
pub(crate) const UNARY_BINDING_POWER: u8 = BP_UNARY;
pub(crate) const POSTFIX_BINDING_POWER: u8 = BP_POSTFIX;

fn assign_op(kind: TokenKind) -> Option<AssignOp> {
    use AssignOp::*;
    Some(match kind {
        T::Equal => Assign,
        T::PlusEqual => Add,
        T::MinusEqual => Sub,
        T::StarEqual => Mul,
        T::SlashEqual => Div,
        T::PercentEqual => Rem,
        T::CaretEqual => Pow,
        T::TildeEqual => Concat,
        T::AndAndEqual => And,
        T::OrOrEqual => Or,
        T::QuestionQuestionEqual => Coalesce,
        _ => return None,
    })
}

struct Parser<'s> {
    source: &'s str,
    tokens: Vec<Token>,
    pos: usize,
}

impl<'s> Parser<'s> {
    fn new(source: &'s str) -> PResult<Parser<'s>> {
        let tokens = lexer::tokenize(source).map_err(|e| ParseError { message: e.message, span: e.span })?;
        Ok(Parser { source, tokens, pos: 0 })
    }

    // ----- Token helpers -----

    fn peek(&self) -> TokenKind {
        self.tokens[self.pos].kind
    }

    fn peek_at(&self, offset: usize) -> TokenKind {
        self.tokens.get(self.pos + offset).map_or(T::Eof, |t| t.kind)
    }

    fn token(&self) -> Token {
        self.tokens[self.pos]
    }

    fn text(&self, token: Token) -> &'s str {
        token.span.text(self.source)
    }

    fn peek_is_word(&self, word: &str) -> bool {
        self.peek() == T::Ident && self.text(self.token()) == word
    }

    fn bump(&mut self) -> Token {
        let token = self.tokens[self.pos];
        if token.kind != T::Eof {
            self.pos += 1;
        }
        token
    }

    fn eat(&mut self, kind: TokenKind) -> Option<Token> {
        if self.peek() == kind { Some(self.bump()) } else { None }
    }

    fn expect(&mut self, kind: TokenKind) -> PResult<Token> {
        if self.peek() == kind {
            Ok(self.bump())
        } else {
            Err(self.error_here(&format!("expected {}", describe_kind(kind))))
        }
    }

    fn error_here(&self, expected: &str) -> ParseError {
        let token = self.token();
        let found = if token.kind == T::Eof { "end of file".to_string() } else { format!("{:?}", self.text(token)) };
        ParseError { message: format!("{expected}, found {found}"), span: token.span }
    }

    fn previous_end(&self) -> u32 {
        self.tokens[self.pos.saturating_sub(1)].span.end
    }

    fn span_from(&self, start: u32) -> Span {
        Span { start, end: self.previous_end().max(start) }
    }

    fn ident(&mut self) -> PResult<Ident> {
        let token = self.expect(T::Ident)?;
        Ok(Ident { name: self.text(token).to_string(), span: token.span })
    }

    fn string_literal(&mut self) -> PResult<String> {
        let token = self.expect(T::String)?;
        lexer::decode_string(self.text(token)).map_err(|message| ParseError { message, span: token.span })
    }

    /// Index of the token after the bracket that closes the one at `open`, or `None` if it isn't closed.
    fn after_matching(&self, open: usize) -> Option<usize> {
        let mut depth = 0usize;
        for (i, token) in self.tokens.iter().enumerate().skip(open) {
            match token.kind {
                T::LParen | T::LBracket | T::LBrace => depth += 1,
                T::RParen | T::RBracket | T::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i + 1);
                    }
                }
                T::Eof => return None,
                _ => {}
            }
        }
        None
    }

    // ----- Module and items -----

    fn module(&mut self) -> PResult<Module> {
        let mut version = None;
        if self.peek_is_word("FeatureScript") && self.peek_at(1) == T::Number {
            self.bump();
            let number = self.bump();
            let text = self.text(number);
            version = Some(text.parse().map_err(|_| ParseError {
                message: format!("bad FeatureScript version {text:?}"),
                span: number.span,
            })?);
            self.expect(T::Semicolon)?;
        }
        let mut items = Vec::new();
        while self.peek() != T::Eof {
            items.push(self.item()?);
        }
        Ok(Module { version, items })
    }

    fn annotations(&mut self) -> PResult<Vec<Expr>> {
        let mut annotations = Vec::new();
        while self.eat(T::Annotation).is_some() {
            if self.peek() != T::LBrace {
                return Err(self.error_here("expected '{' after annotation"));
            }
            annotations.push(self.map_literal()?);
        }
        Ok(annotations)
    }

    fn item(&mut self) -> PResult<Item> {
        let start = self.token().span.start;
        let annotations = self.annotations()?;
        let exported = self.eat(T::Export).is_some();
        let kind = match self.peek() {
            T::Import => self.import(None)?,
            T::Ident if self.peek_at(1) == T::ColonColon && self.peek_at(2) == T::Import => {
                let namespace = self.ident()?;
                self.bump();
                self.import(Some(namespace))?
            }
            T::Const => {
                self.bump();
                let name = self.ident()?;
                let ty = if self.eat(T::Is).is_some() { Some(self.type_name()?) } else { None };
                self.expect(T::Equal)?;
                let value = self.expr()?;
                self.expect(T::Semicolon)?;
                ItemKind::Const { name, ty, value }
            }
            T::Function => {
                let fn_start = self.bump().span.start;
                let name = self.ident()?;
                let function = self.function_rest(fn_start)?;
                ItemKind::Function { name, function }
            }
            T::Predicate => {
                self.bump();
                let name = self.ident()?;
                let params = self.params()?;
                let precondition =
                    if self.eat(T::Precondition).is_some() { Some(Box::new(self.stmt()?)) } else { None };
                let body = self.block()?;
                ItemKind::Predicate { name, params, precondition, body }
            }
            T::Operator => {
                let op_start = self.bump().span.start;
                let op = match self.bump().kind {
                    T::Plus => OverloadableOp::Add,
                    T::Minus => OverloadableOp::Sub,
                    T::Star => OverloadableOp::Mul,
                    T::Slash => OverloadableOp::Div,
                    T::Percent => OverloadableOp::Rem,
                    T::Caret => OverloadableOp::Pow,
                    T::Less => OverloadableOp::Less,
                    _ => {
                        self.pos -= 1;
                        return Err(self.error_here("expected an overloadable operator (+ - * / % ^ <)"));
                    }
                };
                let function = self.function_rest(op_start)?;
                ItemKind::Operator { op, function }
            }
            T::Type => {
                self.bump();
                let name = self.ident()?;
                self.expect(T::Typecheck)?;
                let typecheck = self.path()?;
                self.expect(T::Semicolon)?;
                ItemKind::Type { name, typecheck }
            }
            T::Enum => {
                self.bump();
                let name = self.ident()?;
                self.expect(T::LBrace)?;
                let mut values = Vec::new();
                while self.peek() != T::RBrace {
                    let annotations = self.annotations()?;
                    let name = self.ident()?;
                    values.push(EnumValue { annotations, name });
                    if self.eat(T::Comma).is_none() {
                        break;
                    }
                }
                self.expect(T::RBrace)?;
                ItemKind::Enum { name, values }
            }
            _ => return Err(self.error_here("expected a top-level declaration")),
        };
        Ok(Item { annotations, exported, kind, span: self.span_from(start) })
    }

    fn import(&mut self, namespace: Option<Ident>) -> PResult<ItemKind> {
        self.expect(T::Import)?;
        self.expect(T::LParen)?;
        let mut path = None;
        let mut version = None;
        loop {
            let key = self.ident()?;
            self.expect(T::Colon)?;
            let value = self.string_literal()?;
            match key.name.as_str() {
                "path" => path = Some(value),
                "version" => version = Some(value),
                other => {
                    return Err(ParseError { message: format!("unknown import argument {other:?}"), span: key.span });
                }
            }
            if self.eat(T::Comma).is_none() {
                break;
            }
        }
        self.expect(T::RParen)?;
        self.expect(T::Semicolon)?;
        let missing = |what: &str| ParseError { message: format!("import needs a {what}"), span: self.span_from(0) };
        Ok(ItemKind::Import {
            namespace,
            path: path.ok_or_else(|| missing("path"))?,
            version: version.ok_or_else(|| missing("version"))?,
        })
    }

    /// Parses `(params) returns T precondition ... { body }`, after the `function` keyword and any name.
    fn function_rest(&mut self, start: u32) -> PResult<Function> {
        let params = self.params()?;
        let returns = if self.eat(T::Returns).is_some() { Some(self.type_name()?) } else { None };
        let precondition = if self.eat(T::Precondition).is_some() { Some(Box::new(self.stmt()?)) } else { None };
        let body = FunctionBody::Block(self.block()?);
        Ok(Function { params, returns, precondition, body, span: self.span_from(start) })
    }

    fn params(&mut self) -> PResult<Vec<Param>> {
        self.expect(T::LParen)?;
        let mut params = Vec::new();
        while self.peek() != T::RParen {
            let name = self.ident()?;
            let ty = if self.eat(T::Is).is_some() { Some(self.type_name()?) } else { None };
            params.push(Param { name, ty });
            if self.eat(T::Comma).is_none() {
                break;
            }
        }
        self.expect(T::RParen)?;
        Ok(params)
    }

    fn path(&mut self) -> PResult<Path> {
        let first = self.ident()?;
        if self.peek() == T::ColonColon && self.peek_at(1) == T::Ident {
            self.bump();
            let name = self.ident()?;
            let span = first.span.to(name.span);
            Ok(Path { namespace: Some(first), name, span })
        } else {
            let span = first.span;
            Ok(Path { namespace: None, name: first, span })
        }
    }

    fn type_name(&mut self) -> PResult<TypeName> {
        // `undefined` and `function` are keywords that also name types.
        if matches!(self.peek(), T::Undefined | T::Function) {
            let token = self.bump();
            let name = Ident { name: self.text(token).to_string(), span: token.span };
            return Ok(TypeName { path: Path { namespace: None, name, span: token.span } });
        }
        if self.peek() != T::Ident {
            return Err(self.error_here("expected a type"));
        }
        Ok(TypeName { path: self.path()? })
    }

    // ----- Statements -----

    fn block(&mut self) -> PResult<Block> {
        let start = self.expect(T::LBrace)?.span.start;
        let mut stmts = Vec::new();
        while self.peek() != T::RBrace {
            if self.peek() == T::Eof {
                return Err(self.error_here("expected '}'"));
            }
            stmts.push(self.stmt()?);
        }
        self.bump();
        Ok(Block { stmts, span: self.span_from(start) })
    }

    fn stmt(&mut self) -> PResult<Stmt> {
        let start = self.token().span.start;
        let annotations = self.annotations()?;
        let kind = match self.peek() {
            T::LBrace => StmtKind::Block(self.block()?),
            T::If => {
                self.bump();
                self.expect(T::LParen)?;
                let cond = self.expr()?;
                self.expect(T::RParen)?;
                let then = Box::new(self.stmt()?);
                let otherwise = if self.eat(T::Else).is_some() { Some(Box::new(self.stmt()?)) } else { None };
                StmtKind::If { cond, then, otherwise }
            }
            T::While => {
                self.bump();
                self.expect(T::LParen)?;
                let cond = self.expr()?;
                self.expect(T::RParen)?;
                StmtKind::While { cond, body: Box::new(self.stmt()?) }
            }
            T::For => self.for_stmt()?,
            T::Return => {
                self.bump();
                let value = if self.peek() == T::Semicolon { None } else { Some(self.expr()?) };
                self.expect(T::Semicolon)?;
                StmtKind::Return(value)
            }
            T::Break => {
                self.bump();
                self.expect(T::Semicolon)?;
                StmtKind::Break
            }
            T::Continue => {
                self.bump();
                self.expect(T::Semicolon)?;
                StmtKind::Continue
            }
            T::Throw => {
                self.bump();
                let value = self.expr()?;
                self.expect(T::Semicolon)?;
                StmtKind::Throw(value)
            }
            // `try {` and `try silent {` are statements; `try(...)` and `try silent(...)` are expressions.
            T::Try if self.peek_at(1) == T::LBrace || (self.peek_at(1) == T::Ident && self.peek_at(2) == T::LBrace) => {
                self.bump();
                let silent = if self.peek() == T::Ident {
                    if !self.peek_is_word("silent") {
                        return Err(self.error_here("expected 'silent' or '{' after try"));
                    }
                    self.bump();
                    true
                } else {
                    false
                };
                let body = self.block()?;
                let catch = if self.eat(T::Catch).is_some() {
                    let name = if self.eat(T::LParen).is_some() {
                        let name = self.ident()?;
                        self.expect(T::RParen)?;
                        Some(name)
                    } else {
                        None
                    };
                    Some(Catch { name, body: self.block()? })
                } else {
                    None
                };
                StmtKind::Try { silent, body, catch }
            }
            _ => {
                let kind = self.simple_stmt()?;
                self.expect(T::Semicolon)?;
                kind
            }
        };
        Ok(Stmt { annotations, kind, span: self.span_from(start) })
    }

    /// A declaration, assignment or expression, without its semicolon (also used in `for (;;)` headers).
    fn simple_stmt(&mut self) -> PResult<StmtKind> {
        if let Some(kind) = match self.peek() {
            T::Var => Some(DeclKind::Var),
            T::Const => Some(DeclKind::Const),
            _ => None,
        } {
            self.bump();
            let name = self.ident()?;
            let ty = if self.eat(T::Is).is_some() { Some(self.type_name()?) } else { None };
            let init = if self.eat(T::Equal).is_some() { Some(self.expr()?) } else { None };
            return Ok(StmtKind::Decl { kind, name, ty, init });
        }
        let expr = self.expr()?;
        if let Some(op) = assign_op(self.peek()) {
            let op_token = self.bump();
            if !matches!(
                expr.kind,
                ExprKind::Name(_)
                    | ExprKind::Member { optional: false, .. }
                    | ExprKind::Index { .. }
                    | ExprKind::BoxValue(_)
            ) {
                return Err(ParseError { message: "can't assign to this expression".into(), span: op_token.span });
            }
            let value = self.expr()?;
            return Ok(StmtKind::Assign { target: expr, op, value });
        }
        Ok(StmtKind::Expr(expr))
    }

    fn for_stmt(&mut self) -> PResult<StmtKind> {
        self.expect(T::For)?;
        self.expect(T::LParen)?;

        // for-in: `[var|const] a [, b] in x`
        let decl_offset = usize::from(matches!(self.peek(), T::Var | T::Const));
        let is_for_in = self.peek_at(decl_offset) == T::Ident
            && (self.peek_at(decl_offset + 1) == T::In
                || (self.peek_at(decl_offset + 1) == T::Comma
                    && self.peek_at(decl_offset + 2) == T::Ident
                    && self.peek_at(decl_offset + 3) == T::In));
        if is_for_in {
            let decl = match self.peek() {
                T::Var => Some(DeclKind::Var),
                T::Const => Some(DeclKind::Const),
                _ => None,
            };
            if decl.is_some() {
                self.bump();
            }
            let first = self.ident()?;
            let (key, value) = if self.eat(T::Comma).is_some() { (Some(first), self.ident()?) } else { (None, first) };
            self.expect(T::In)?;
            let iterable = self.expr()?;
            self.expect(T::RParen)?;
            let body = Box::new(self.stmt()?);
            return Ok(StmtKind::ForIn { decl, key, value, iterable, body });
        }

        let init = self.optional_simple_stmt(T::Semicolon)?;
        self.expect(T::Semicolon)?;
        let cond = if self.peek() == T::Semicolon { None } else { Some(self.expr()?) };
        self.expect(T::Semicolon)?;
        let step = self.optional_simple_stmt(T::RParen)?;
        self.expect(T::RParen)?;
        let body = Box::new(self.stmt()?);
        Ok(StmtKind::For { init, cond, step, body })
    }

    fn optional_simple_stmt(&mut self, terminator: TokenKind) -> PResult<Option<Box<Stmt>>> {
        if self.peek() == terminator {
            return Ok(None);
        }
        let start = self.token().span.start;
        let kind = self.simple_stmt()?;
        Ok(Some(Box::new(Stmt { annotations: Vec::new(), kind, span: self.span_from(start) })))
    }

    // ----- Expressions -----

    fn expr(&mut self) -> PResult<Expr> {
        self.expr_bp(0)
    }

    fn expr_bp(&mut self, min_bp: u8) -> PResult<Expr> {
        let mut lhs = self.prefix()?;
        loop {
            let kind = self.peek();
            match kind {
                T::LParen | T::LBracket | T::Dot | T::QuestionDot | T::Arrow => {
                    if BP_POSTFIX < min_bp {
                        break;
                    }
                    lhs = self.postfix(lhs)?;
                }
                T::Is | T::As => {
                    if BP_TYPE_TEST < min_bp {
                        break;
                    }
                    self.bump();
                    let ty = self.type_name()?;
                    let span = lhs.span.to(ty.path.span);
                    let operand = Box::new(lhs);
                    let kind = if kind == T::Is { ExprKind::Is { operand, ty } } else { ExprKind::As { operand, ty } };
                    lhs = Expr { kind, span };
                }
                T::Question => {
                    if BP_TERNARY < min_bp {
                        break;
                    }
                    self.bump();
                    let then = self.expr_bp(0)?;
                    self.expect(T::Colon)?;
                    let otherwise = self.expr_bp(BP_TERNARY)?;
                    let span = lhs.span.to(otherwise.span);
                    lhs = Expr {
                        kind: ExprKind::Ternary {
                            cond: Box::new(lhs),
                            then: Box::new(then),
                            otherwise: Box::new(otherwise),
                        },
                        span,
                    };
                }
                _ => {
                    let Some((op, left_bp, right_bp)) = binary_op(kind) else { break };
                    if left_bp < min_bp {
                        break;
                    }
                    self.bump();
                    let rhs = self.expr_bp(right_bp)?;
                    let span = lhs.span.to(rhs.span);
                    lhs = Expr { kind: ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) }, span };
                }
            }
        }
        Ok(lhs)
    }

    fn postfix(&mut self, lhs: Expr) -> PResult<Expr> {
        let start = lhs.span.start;
        let kind = match self.peek() {
            T::LParen => {
                let args = self.args()?;
                ExprKind::Call { callee: Box::new(lhs), args }
            }
            T::LBracket => {
                self.bump();
                if self.eat(T::RBracket).is_some() {
                    ExprKind::BoxValue(Box::new(lhs))
                } else {
                    let index = self.expr()?;
                    self.expect(T::RBracket)?;
                    ExprKind::Index { target: Box::new(lhs), index: Box::new(index) }
                }
            }
            T::Dot | T::QuestionDot => {
                let optional = self.bump().kind == T::QuestionDot;
                let name = self.ident()?;
                ExprKind::Member { target: Box::new(lhs), name, optional }
            }
            T::Arrow => {
                self.bump();
                let function = self.path()?;
                let args = self.args()?;
                ExprKind::Pipe { receiver: Box::new(lhs), function, args }
            }
            _ => unreachable!("postfix called on a non-postfix token"),
        };
        Ok(Expr { kind, span: self.span_from(start) })
    }

    fn args(&mut self) -> PResult<Vec<Expr>> {
        self.expect(T::LParen)?;
        let mut args = Vec::new();
        while self.peek() != T::RParen {
            args.push(self.expr()?);
            if self.eat(T::Comma).is_none() {
                break;
            }
        }
        self.expect(T::RParen)?;
        Ok(args)
    }

    fn prefix(&mut self) -> PResult<Expr> {
        let token = self.token();
        let start = token.span.start;
        let kind = match token.kind {
            T::Number => {
                self.bump();
                let text = self.text(token);
                let value = text
                    .parse::<f64>()
                    .map_err(|_| ParseError { message: format!("bad number {text:?}"), span: token.span })?;
                ExprKind::Number(value)
            }
            T::String => ExprKind::String(self.string_literal()?),
            T::True => {
                self.bump();
                ExprKind::Bool(true)
            }
            T::False => {
                self.bump();
                ExprKind::Bool(false)
            }
            T::Undefined => {
                self.bump();
                ExprKind::Undefined
            }
            T::Inf => {
                self.bump();
                ExprKind::Inf
            }
            T::Builtin => {
                self.bump();
                ExprKind::Builtin(self.text(token)[1..].to_string())
            }
            T::Ident if self.peek_at(1) == T::FatArrow => {
                let name = self.ident()?;
                return self.lambda_after_params(start, vec![Param { name, ty: None }], None);
            }
            T::Ident => ExprKind::Name(self.path()?),
            T::LParen => {
                let after = self.after_matching(self.pos);
                if after.is_some_and(|i| matches!(self.tokens[i].kind, T::FatArrow | T::Returns)) {
                    let params = self.params()?;
                    let returns = if self.eat(T::Returns).is_some() { Some(self.type_name()?) } else { None };
                    return self.lambda_after_params(start, params, returns);
                }
                self.bump();
                let inner = self.expr()?;
                self.expect(T::RParen)?;
                // Parentheses only group; keep the inner node but widen its span to cover them.
                return Ok(Expr { kind: inner.kind, span: self.span_from(start) });
            }
            T::LBracket => {
                self.bump();
                let mut items = Vec::new();
                while self.peek() != T::RBracket {
                    items.push(self.expr()?);
                    if self.eat(T::Comma).is_none() {
                        break;
                    }
                }
                self.expect(T::RBracket)?;
                ExprKind::Array(items)
            }
            T::LBrace => return self.map_literal(),
            T::Minus | T::Bang => {
                self.bump();
                let operand = self.expr_bp(BP_UNARY)?;
                let op = if token.kind == T::Minus { UnaryOp::Neg } else { UnaryOp::Not };
                ExprKind::Unary { op, operand: Box::new(operand) }
            }
            T::Function => {
                self.bump();
                let function = self.function_rest(start)?;
                ExprKind::Lambda(Box::new(function))
            }
            T::New => {
                self.bump();
                if !self.peek_is_word("box") {
                    return Err(self.error_here("expected 'box' after new"));
                }
                self.bump();
                self.expect(T::LParen)?;
                let value = self.expr()?;
                self.expect(T::RParen)?;
                ExprKind::NewBox(Box::new(value))
            }
            T::Switch => {
                self.bump();
                self.expect(T::LParen)?;
                let value = self.expr()?;
                self.expect(T::RParen)?;
                self.expect(T::LBrace)?;
                let mut cases = Vec::new();
                while self.peek() != T::RBrace {
                    let key = self.expr_bp(BP_KEY)?;
                    self.expect(T::Colon)?;
                    let result = self.expr()?;
                    cases.push((key, result));
                    if self.eat(T::Comma).is_none() {
                        break;
                    }
                }
                self.expect(T::RBrace)?;
                ExprKind::Switch { value: Box::new(value), cases }
            }
            T::Try => {
                self.bump();
                let silent = if self.peek_is_word("silent") {
                    self.bump();
                    true
                } else {
                    false
                };
                // `try (expr)` takes exactly the parenthesised expression, so `try(f()).x` reads the result's x.
                let expr = if self.peek() == T::LParen {
                    self.bump();
                    let inner = self.expr()?;
                    self.expect(T::RParen)?;
                    inner
                } else {
                    self.expr_bp(BP_UNARY)?
                };
                ExprKind::Try { silent, expr: Box::new(expr) }
            }
            _ => return Err(self.error_here("expected an expression")),
        };
        Ok(Expr { kind, span: self.span_from(start) })
    }

    fn lambda_after_params(&mut self, start: u32, params: Vec<Param>, returns: Option<TypeName>) -> PResult<Expr> {
        self.expect(T::FatArrow)?;
        let body = if self.peek() == T::LBrace && !self.brace_starts_map() {
            FunctionBody::Block(self.block()?)
        } else {
            FunctionBody::Expr(Box::new(self.expr()?))
        };
        let function = Function { params, returns, precondition: None, body, span: self.span_from(start) };
        Ok(Expr { kind: ExprKind::Lambda(Box::new(function)), span: self.span_from(start) })
    }

    /// After `=>`, decides whether `{` opens a map literal or a block. It is a map if a `:` comes before any `;`,
    /// `?` or the closing brace at the top nesting level, or if the braces are empty.
    fn brace_starts_map(&self) -> bool {
        let mut depth = 0usize;
        for token in &self.tokens[self.pos + 1..] {
            match token.kind {
                T::LParen | T::LBracket | T::LBrace => depth += 1,
                T::RParen | T::RBracket => depth = depth.saturating_sub(1),
                T::RBrace if depth == 0 => return true,
                T::RBrace => depth -= 1,
                T::Colon if depth == 0 => return true,
                T::Semicolon | T::Question if depth == 0 => return false,
                T::Eof => return false,
                _ => {}
            }
        }
        false
    }

    fn map_literal(&mut self) -> PResult<Expr> {
        let start = self.expect(T::LBrace)?.span.start;
        let mut entries = Vec::new();
        while self.peek() != T::RBrace {
            let key = if self.peek() == T::Ident && self.peek_at(1) == T::Colon {
                MapKey::Name(self.ident()?)
            } else {
                MapKey::Expr(self.expr_bp(BP_KEY)?)
            };
            self.expect(T::Colon)?;
            let value = self.expr()?;
            entries.push(MapEntry { key, value });
            if self.eat(T::Comma).is_none() {
                break;
            }
        }
        self.expect(T::RBrace)?;
        Ok(Expr { kind: ExprKind::Map(entries), span: self.span_from(start) })
    }
}

fn describe_kind(kind: TokenKind) -> String {
    use TokenKind::*;
    let text = match kind {
        Ident => "a name",
        Builtin => "a builtin",
        Number => "a number",
        String => "a string",
        LParen => "'('",
        RParen => "')'",
        LBrace => "'{'",
        RBrace => "'}'",
        LBracket => "'['",
        RBracket => "']'",
        Comma => "','",
        Semicolon => "';'",
        Colon => "':'",
        Equal => "'='",
        FatArrow => "'=>'",
        Eof => "end of file",
        other => return format!("{other:?}").to_lowercase(),
    };
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A compact, fully parenthesised rendering that shows how an expression groups.
    fn sexpr(expr: &Expr) -> String {
        let list = |items: &[Expr]| items.iter().map(sexpr).collect::<Vec<_>>().join(" ");
        let path = |p: &Path| match &p.namespace {
            Some(ns) => format!("{}::{}", ns.name, p.name.name),
            None => p.name.name.clone(),
        };
        match &expr.kind {
            ExprKind::Number(n) => format!("{n}"),
            ExprKind::String(s) => format!("{s:?}"),
            ExprKind::Bool(b) => format!("{b}"),
            ExprKind::Undefined => "undefined".into(),
            ExprKind::Inf => "inf".into(),
            ExprKind::Name(p) => path(p),
            ExprKind::Builtin(name) => format!("@{name}"),
            ExprKind::Array(items) => format!("[{}]", list(items)),
            ExprKind::Map(entries) => {
                let entries: Vec<String> = entries
                    .iter()
                    .map(|e| match &e.key {
                        MapKey::Name(n) => format!("{}:{}", n.name, sexpr(&e.value)),
                        MapKey::Expr(k) => format!("({}):{}", sexpr(k), sexpr(&e.value)),
                    })
                    .collect();
                format!("{{{}}}", entries.join(" "))
            }
            ExprKind::Unary { op, operand } => format!("({op:?} {})", sexpr(operand)),
            ExprKind::Binary { op, lhs, rhs } => format!("({op:?} {} {})", sexpr(lhs), sexpr(rhs)),
            ExprKind::Ternary { cond, then, otherwise } => {
                format!("(? {} {} {})", sexpr(cond), sexpr(then), sexpr(otherwise))
            }
            ExprKind::Is { operand, ty } => format!("(is {} {})", sexpr(operand), path(&ty.path)),
            ExprKind::As { operand, ty } => format!("(as {} {})", sexpr(operand), path(&ty.path)),
            ExprKind::Call { callee, args } => format!("(call {} {})", sexpr(callee), list(args)),
            ExprKind::Pipe { receiver, function, args } => {
                format!("(pipe {} {} {})", sexpr(receiver), path(function), list(args))
            }
            ExprKind::Index { target, index } => format!("(index {} {})", sexpr(target), sexpr(index)),
            ExprKind::Member { target, name, optional } => {
                format!("({} {} {})", if *optional { "?." } else { "." }, sexpr(target), name.name)
            }
            ExprKind::BoxValue(target) => format!("(box-value {})", sexpr(target)),
            ExprKind::NewBox(value) => format!("(new-box {})", sexpr(value)),
            ExprKind::Lambda(function) => {
                let params: Vec<&str> = function.params.iter().map(|p| p.name.name.as_str()).collect();
                match &function.body {
                    FunctionBody::Expr(body) => format!("(lambda ({}) {})", params.join(" "), sexpr(body)),
                    FunctionBody::Block(block) => {
                        format!("(lambda ({}) {{{} stmts}})", params.join(" "), block.stmts.len())
                    }
                }
            }
            ExprKind::Switch { value, cases } => {
                let cases: Vec<String> = cases.iter().map(|(k, v)| format!("{}:{}", sexpr(k), sexpr(v))).collect();
                format!("(switch {} {})", sexpr(value), cases.join(" "))
            }
            ExprKind::Try { silent, expr } => {
                format!("({} {})", if *silent { "try-silent" } else { "try" }, sexpr(expr))
            }
        }
    }

    fn check(source: &str, expected: &str) {
        let expr = parse_expr(source).unwrap_or_else(|e| panic!("{source}: {}", e.describe(source)));
        assert_eq!(sexpr(&expr), expected, "{source}");
    }

    #[test]
    fn arithmetic_precedence() {
        check("1 + 2 * 3", "(Add 1 (Mul 2 3))");
        check("1 - 2 - 3", "(Sub (Sub 1 2) 3)");
        check("2 ^ 3 ^ 2", "(Pow 2 (Pow 3 2))");
        check("-x ^ 2", "(Neg (Pow x 2))");
        check("2 ^ -1", "(Pow 2 (Neg 1))");
        check("-a.b", "(Neg (. a b))");
        check("\"a\" ~ 1 + 2", "(Concat \"a\" (Add 1 2))");
    }

    #[test]
    fn logic_and_type_tests() {
        check("a || b && c", "(Or a (And b c))");
        check("x is string == true", "(Equal (is x string) true)");
        check("a + b is number", "(is (Add a b) number)");
        check("x is number && y is Vector", "(And (is x number) (is y Vector))");
        check("v is undefined", "(is v undefined)");
        check("{ 'minCorner' : a } as Box3d", "(as {(\"minCorner\"):a} Box3d)");
        check("a ?? b || c", "(Coalesce a (Or b c))");
        check("c ? a : d ? e : f", "(? c a (? d e f))");
        check("c ?.5 : 1", "(? c 0.5 1)");
    }

    #[test]
    fn postfix_forms() {
        check("f(a)(b)[c].d[]", "(box-value (. (index (call (call f a) b) c) d))");
        check("q->qBodyType(BodyType.WIRE)->f()", "(pipe (pipe q qBodyType (. BodyType WIRE)) f )");
        check("a?.angle?.value", "(?. (?. a angle) value)");
        check("@opExtrude(context, id, d)", "(call @opExtrude context id d)");
        check("ns::f(x)", "(call ns::f x)");
        check("new box(0)", "(new-box 0)");
    }

    #[test]
    fn maps_and_keys() {
        check("{ a : 1, \"b\" : 2, (c) : 3, E.X : 4, }", "{a:1 (\"b\"):2 (c):3 ((. E X)):4}");
        check("{}", "{}");
    }

    #[test]
    fn lambdas() {
        check("x => x ^ 2", "(lambda (x) (Pow x 2))");
        check("(a, b) => a * b", "(lambda (a b) (Mul a b))");
        check("index => { \"index\" : index }", "(lambda (index) {(\"index\"):index})");
        check("(a is number) returns number => { return a; }", "(lambda (a) {1 stmts})");
        check("e => { const c = e; return c; }", "(lambda (e) {2 stmts})");
        check("function(x) { return x; }", "(lambda (x) {1 stmts})");
        check("mapArray(a, v => v * m)", "(call mapArray a (lambda (v) (Mul v m)))");
    }

    #[test]
    fn switch_and_try() {
        check("switch (m) { T.A : 0, T.B : 1 }", "(switch m (. T A):0 (. T B):1)");
        check("try silent(f(x))", "(try-silent (call f x))");
        check("try(f(x)).y", "(. (try (call f x)) y)");
    }

    #[test]
    fn statements() {
        let ok = [
            "for (var i = 0; i < n; i += 1) {}",
            "for (var k, v in m) {}",
            "for (element in y) element is T;",
            "try { f(); } catch (e) { g(); }",
            "try silent { f(); }",
            "try { f(); } catch { g(); }",
            "b[] = 0;",
            "a.b.c[0] = 1;",
            "x ??= 1;",
            "annotation { \"Name\" : \"Depth\" } isLength(definition.depth, BOUNDS);",
            "if (a) return 1; else if (b) return 2; else return;",
        ];
        for source in ok {
            parse_stmt(source).unwrap_or_else(|e| panic!("{source}: {}", e.describe(source)));
        }
        assert!(parse_stmt("f() = 1;").is_err());
        assert!(parse_stmt("a?.b = 1;").is_err());
    }

    #[test]
    fn error_positions() {
        let source = "FeatureScript 1;\nexport const x = ;";
        let error = parse_module(source).unwrap_err();
        assert_eq!(error.describe(source), "2:18: expected an expression, found \";\"");
    }
}
