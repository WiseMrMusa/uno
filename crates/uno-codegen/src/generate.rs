use uno_syntax::ast::*;
use uno_syntax::span::Span;

pub trait ExprGen {
    type Output;
    type Error;
    fn gen_literal(&mut self, val: &str, span: &Span) -> Result<Self::Output, Self::Error>;
    fn gen_ident(&mut self, name: &str, span: &Span) -> Result<Self::Output, Self::Error>;
    fn gen_binary_op(
        &mut self,
        left: &Expr,
        op: &BinOp,
        right: &Expr,
        span: &Span,
    ) -> Result<Self::Output, Self::Error>;
    fn gen_unary_op(
        &mut self,
        op: &UnOp,
        operand: &Expr,
        span: &Span,
    ) -> Result<Self::Output, Self::Error>;
    fn gen_block_expr(&mut self, block: &Block) -> Result<Self::Output, Self::Error>;
    fn gen_fn_call(
        &mut self,
        name: &str,
        args: &[Expr],
        span: &Span,
    ) -> Result<Self::Output, Self::Error>;
    fn gen_paren(
        &mut self,
        inner: &Expr,
        span: &Span,
    ) -> Result<Self::Output, Self::Error>;
}

pub trait StmtGen {
    type Output;
    type Error;
    fn gen_let_stmt(
        &mut self,
        name: &str,
        is_mut: bool,
        type_: Option<&Type>,
        value: &Expr,
        span: &Span,
    ) -> Result<Self::Output, Self::Error>;
    fn gen_return_stmt(
        &mut self,
        expr: Option<&Expr>,
        span: &Span,
    ) -> Result<Self::Output, Self::Error>;
    fn gen_expr_stmt(
        &mut self,
        expr: &Expr,
        span: &Span,
    ) -> Result<Self::Output, Self::Error>;
    fn gen_if_stmt(
        &mut self,
        cond: &Expr,
        then_block: &Block,
        else_block: Option<&Block>,
        span: &Span,
    ) -> Result<Self::Output, Self::Error>;
}

pub trait TypeGen {
    type Output;
    type Error;
    fn gen_type(&mut self, type_: &Type) -> Result<Self::Output, Self::Error>;
}

pub trait FnGen {
    type Output;
    type Error;
    fn gen_fn_def(&mut self, fn_def: &FnDef) -> Result<Self::Output, Self::Error>;
}

pub trait ProgramGen {
    type Output;
    type Error;
    fn gen_program(&mut self, prog: &Program) -> Result<Self::Output, Self::Error>;
}
