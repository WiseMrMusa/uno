use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Neq,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Bool,
    Uint(usize),
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(String, Span),
    Ident(String, Span),
    BinaryOp(Box<Expr>, BinOp, Box<Expr>, Span),
    UnaryOp(UnOp, Box<Expr>, Span),
    Block(Block, Span),
    FnCall(String, Vec<Expr>, Span),
    Paren(Box<Expr>, Span),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let(String, bool, Option<Type>, Expr, Span),
    Assign(String, Expr, Span),
    Return(Option<Expr>, Span),
    Expr(Expr, Span),
    If(Expr, Block, Option<Block>, Span),
    While(Expr, Block, Span),
    Loop(Block, Span),
    Break(Span),
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub type_: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FnDef {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub body: Block,
    pub span: Span,
    pub public: bool,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub imports: Vec<String>,
    pub functions: Vec<FnDef>,
}
