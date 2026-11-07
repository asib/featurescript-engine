//! FeatureScript source handling: tokens, syntax tree, parser and printer.

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod span;

pub use parser::{ParseError, parse_expr, parse_module, parse_stmt};
pub use span::Span;
