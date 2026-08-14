/// Token kinds produced by the lexer. Kept as an owned enum (rather than a
/// `(kind, slice)` pair) so tests can assert on exact token sequences without
/// juggling lifetimes.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Number(f64),
    Str(String),
    Ident(String),

    // Keywords
    Let,
    Fn,
    If,
    Else,
    While,
    For,
    Return,
    True,
    False,
    Nil,
    Print,

    // Punctuation
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    Colon,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Assign,
    EqEq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    AndAnd,
    OrOr,
    Bang,

    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: u32,
}

impl Token {
    pub fn new(kind: TokenKind, line: u32) -> Self {
        Token { kind, line }
    }
}
