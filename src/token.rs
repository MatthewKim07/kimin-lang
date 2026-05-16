#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Number(f64),
    String(String),
    True,
    False,

    // Identifiers and keywords
    Ident(String),
    Let,
    If,
    Else,
    Print,
    Fn,
    Return,
    State,
    Transition,
    Simulate,
    Step,

    // Arithmetic operators
    Plus,
    Minus,
    Star,
    Slash,

    // Comparison and logical operators
    Bang,
    BangEq,
    Eq,
    EqEq,
    Lt,
    LtEq,
    Gt,
    GtEq,

    // Delimiters
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Arrow,
    Dot,

    Eof,
}

/// Source location for error reporting and future span-aware features.
#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, line: usize, col: usize) -> Self {
        Token {
            kind,
            span: Span { line, col },
        }
    }
}
