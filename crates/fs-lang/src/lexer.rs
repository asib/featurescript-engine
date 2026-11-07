//! Turns FeatureScript source into tokens.
//!
//! Follows the lexical conventions in Onshape's FeatureScript guide (`FsDoc/tokens.html`). Comments and whitespace
//! are dropped. String tokens keep their quotes; [`decode_string`] turns one into its value.

use crate::span::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokenKind {
    Ident,
    /// `@name`: a function provided by the runtime.
    Builtin,
    Number,
    String,

    // Keywords.
    Annotation,
    As,
    Break,
    Catch,
    Const,
    Continue,
    Else,
    Enum,
    Export,
    False,
    For,
    Function,
    If,
    Import,
    In,
    Inf,
    Is,
    New,
    Operator,
    Precondition,
    Predicate,
    Return,
    Returns,
    Switch,
    Throw,
    True,
    Try,
    Type,
    Typecheck,
    Typeconvert,
    BoxKw,
    Undefined,
    Var,
    While,

    // Punctuation.
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    Colon,
    ColonColon,
    Dot,
    Question,
    QuestionDot,
    QuestionQuestion,
    Arrow,
    FatArrow,

    // Operators.
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Tilde,
    Bang,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    EqualEqual,
    BangEqual,
    AndAnd,
    OrOr,

    // Assignment.
    Equal,
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    PercentEqual,
    CaretEqual,
    TildeEqual,
    AndAndEqual,
    OrOrEqual,
    QuestionQuestionEqual,

    Eof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

pub fn keyword(word: &str) -> Option<TokenKind> {
    use TokenKind::*;
    Some(match word {
        "annotation" => Annotation,
        "as" => As,
        "box" => BoxKw,
        "break" => Break,
        "catch" => Catch,
        "const" => Const,
        "continue" => Continue,
        "else" => Else,
        "enum" => Enum,
        "export" => Export,
        "false" => False,
        "for" => For,
        "function" => Function,
        "if" => If,
        "import" => Import,
        "in" => In,
        "inf" => Inf,
        "is" => Is,
        "new" => New,
        "operator" => Operator,
        "precondition" => Precondition,
        "predicate" => Predicate,
        "return" => Return,
        "returns" => Returns,
        "switch" => Switch,
        "throw" => Throw,
        "true" => True,
        "try" => Try,
        "type" => Type,
        "typecheck" => Typecheck,
        "typeconvert" => Typeconvert,
        "undefined" => Undefined,
        "var" => Var,
        "while" => While,
        _ => return None,
    })
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

pub fn tokenize(source: &str) -> Result<Vec<Token>, LexError> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::with_capacity(source.len() / 4);
    let mut i = 0;
    let at = |j: usize| -> u8 { bytes.get(j).copied().unwrap_or(0) };

    while i < bytes.len() {
        let c = bytes[i];
        // Whitespace and comments.
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c == b'/' && at(i + 1) == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && at(i + 1) == b'*' {
            let start = i;
            i += 2;
            loop {
                if i + 1 >= bytes.len() {
                    return Err(LexError { message: "unterminated comment".into(), span: Span::new(start, i) });
                }
                if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            continue;
        }

        let start = i;
        let kind = if is_ident_start(c) {
            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }
            keyword(&source[start..i]).unwrap_or(TokenKind::Ident)
        } else if c == b'@' {
            i += 1;
            if !is_ident_start(at(i)) {
                return Err(LexError { message: "expected a name after '@'".into(), span: Span::new(start, i) });
            }
            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }
            TokenKind::Builtin
        } else if c.is_ascii_digit() || (c == b'.' && at(i + 1).is_ascii_digit()) {
            i = lex_number(bytes, i);
            if is_ident_start(at(i)) {
                return Err(LexError {
                    message: "a number can't run into a name".into(),
                    span: Span::new(start, i + 1),
                });
            }
            TokenKind::Number
        } else if c == b'"' || c == b'\'' {
            i += 1;
            loop {
                match at(i) {
                    0 if i >= bytes.len() => {
                        return Err(LexError { message: "unterminated string".into(), span: Span::new(start, i) });
                    }
                    b'\n' => {
                        return Err(LexError { message: "newline in string".into(), span: Span::new(start, i) });
                    }
                    b'\\' => i += 2,
                    q if q == c => {
                        i += 1;
                        break;
                    }
                    _ => i += 1,
                }
            }
            TokenKind::String
        } else {
            let (kind, len) = punctuation(&bytes[i..]).ok_or_else(|| LexError {
                message: format!("unexpected character {:?}", source[i..].chars().next().unwrap()),
                span: Span::new(i, i + 1),
            })?;
            i += len;
            kind
        };
        tokens.push(Token { kind, span: Span::new(start, i) });
    }
    tokens.push(Token { kind: TokenKind::Eof, span: Span::new(bytes.len(), bytes.len()) });
    Ok(tokens)
}

/// Lexes a decimal number such as `3`, `1.44`, `.5`, `2.` or `3e-9`, returning the index after it.
fn lex_number(bytes: &[u8], mut i: usize) -> usize {
    let at = |j: usize| -> u8 { bytes.get(j).copied().unwrap_or(0) };
    while at(i).is_ascii_digit() {
        i += 1;
    }
    if at(i) == b'.' {
        let next = at(i + 1);
        // `1.` and `1.e5` are numbers, but in `1.foo` the dot would be member access.
        let exponent_follows = (next == b'e' || next == b'E')
            && (at(i + 2).is_ascii_digit() || ((at(i + 2) == b'-' || at(i + 2) == b'+') && at(i + 3).is_ascii_digit()));
        if next.is_ascii_digit() || exponent_follows || !is_ident_start(next) && next != b'.' {
            i += 1;
            while at(i).is_ascii_digit() {
                i += 1;
            }
        }
    }
    if at(i) == b'e' || at(i) == b'E' {
        let mut j = i + 1;
        if at(j) == b'-' || at(j) == b'+' {
            j += 1;
        }
        if at(j).is_ascii_digit() {
            i = j;
            while at(i).is_ascii_digit() {
                i += 1;
            }
        }
    }
    i
}

fn punctuation(rest: &[u8]) -> Option<(TokenKind, usize)> {
    use TokenKind::*;
    let c0 = rest[0];
    let c1 = rest.get(1).copied().unwrap_or(0);
    let c2 = rest.get(2).copied().unwrap_or(0);
    Some(match (c0, c1, c2) {
        (b'?', b'?', b'=') => (QuestionQuestionEqual, 3),
        (b'&', b'&', b'=') => (AndAndEqual, 3),
        (b'|', b'|', b'=') => (OrOrEqual, 3),
        (b'?', b'?', _) => (QuestionQuestion, 2),
        // `cond ?.5 : 1` is a ternary with the number .5, not optional member access.
        (b'?', b'.', d) if !d.is_ascii_digit() => (QuestionDot, 2),
        (b'&', b'&', _) => (AndAnd, 2),
        (b'|', b'|', _) => (OrOr, 2),
        (b':', b':', _) => (ColonColon, 2),
        (b'-', b'>', _) => (Arrow, 2),
        (b'=', b'>', _) => (FatArrow, 2),
        (b'=', b'=', _) => (EqualEqual, 2),
        (b'!', b'=', _) => (BangEqual, 2),
        (b'<', b'=', _) => (LessEqual, 2),
        (b'>', b'=', _) => (GreaterEqual, 2),
        (b'+', b'=', _) => (PlusEqual, 2),
        (b'-', b'=', _) => (MinusEqual, 2),
        (b'*', b'=', _) => (StarEqual, 2),
        (b'/', b'=', _) => (SlashEqual, 2),
        (b'%', b'=', _) => (PercentEqual, 2),
        (b'^', b'=', _) => (CaretEqual, 2),
        (b'~', b'=', _) => (TildeEqual, 2),
        (b'(', _, _) => (LParen, 1),
        (b')', _, _) => (RParen, 1),
        (b'{', _, _) => (LBrace, 1),
        (b'}', _, _) => (RBrace, 1),
        (b'[', _, _) => (LBracket, 1),
        (b']', _, _) => (RBracket, 1),
        (b',', _, _) => (Comma, 1),
        (b';', _, _) => (Semicolon, 1),
        (b':', _, _) => (Colon, 1),
        (b'.', _, _) => (Dot, 1),
        (b'?', _, _) => (Question, 1),
        (b'+', _, _) => (Plus, 1),
        (b'-', _, _) => (Minus, 1),
        (b'*', _, _) => (Star, 1),
        (b'/', _, _) => (Slash, 1),
        (b'%', _, _) => (Percent, 1),
        (b'^', _, _) => (Caret, 1),
        (b'~', _, _) => (Tilde, 1),
        (b'!', _, _) => (Bang, 1),
        (b'<', _, _) => (Less, 1),
        (b'>', _, _) => (Greater, 1),
        (b'=', _, _) => (Equal, 1),
        _ => return None,
    })
}

/// Returns the value of a string token, given its text including the quotes.
///
/// Recognises the escapes in the FeatureScript guide (`\b \t \n \f \r`, `\uXXXX`) plus an escaped quote or
/// backslash. Any other escaped character stands for itself.
pub fn decode_string(token_text: &str) -> Result<String, String> {
    let inner = &token_text[1..token_text.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('b') => out.push('\u{8}'),
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('f') => out.push('\u{c}'),
            Some('r') => out.push('\r'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                let code = u32::from_str_radix(&hex, 16).map_err(|_| format!("bad \\u escape {hex:?}"))?;
                out.push(char::from_u32(code).ok_or_else(|| format!("bad \\u escape {hex:?}"))?);
            }
            Some(other) => out.push(other),
            None => return Err("string ends with a backslash".into()),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use TokenKind::*;

    fn kinds(source: &str) -> Vec<TokenKind> {
        tokenize(source).unwrap().into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn numbers() {
        for text in ["3", "1.44", ".5", "2.", "3e9", "1e-8", "1.e-5", "1.5E+3"] {
            let tokens = tokenize(text).unwrap();
            assert_eq!(tokens[0].kind, Number, "{text}");
            assert_eq!(tokens[0].span.text(text), text);
        }
        assert_eq!(kinds("a[1].b"), [Ident, LBracket, Number, RBracket, Dot, Ident, Eof]);
    }

    #[test]
    fn question_dot_before_digit_is_a_ternary() {
        assert_eq!(kinds("a ?.5 : 1"), [Ident, Question, Number, Colon, Number, Eof]);
        assert_eq!(kinds("a?.b"), [Ident, QuestionDot, Ident, Eof]);
    }

    #[test]
    fn comments_and_operators() {
        assert_eq!(
            kinds("x /* c */ ??= y // d\n-> => :: @opExtrude ~= ||="),
            [Ident, QuestionQuestionEqual, Ident, Arrow, FatArrow, ColonColon, Builtin, TildeEqual, OrOrEqual, Eof]
        );
    }

    #[test]
    fn strings() {
        assert_eq!(decode_string(r#""a\"b\\c\nd""#).unwrap(), "a\"b\\c\nd");
        assert_eq!(decode_string(r"'it\'s é'").unwrap(), "it's é");
        assert!(tokenize("\"open").is_err());
    }
}
