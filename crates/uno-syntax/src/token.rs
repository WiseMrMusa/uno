use crate::span::Span;

#[derive(Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Fn,
    Let,
    Return,
    If,
    Else,
    True,
    False,
    Mut,
    Pub,
    While,
    Loop,
    Break,
    For,
    In,
    Match,
    Struct,
    Enum,
    Impl,
    Use,

    BoolType,
    Uint(usize),

    Integer(String),
    Ident(String),

    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Equals,
    EqualsEquals,
    NotEquals,
    Less,
    Greater,
    LessEquals,
    GreaterEquals,
    Not,
    AndAnd,
    OrOr,

    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    Colon,
    Arrow,
    Dot,
    Underscore,
    Hash,

    Error(String),
    Eof,
    
}