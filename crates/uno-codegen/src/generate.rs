use uno_syntax::ast::*;
use uno_syntax::span::Span;

/// One method per expression variant.
/// When every method is implemented, all possible expressions are handled.
pub trait ExprGen {
    type Output;
    fn gen_literal(&mut self, val: &str, span: &Span) -> Self::Output;
    fn gen_ident(&mut self, name: &str, span: &Span) -> Self::Output;
    fn gen_binary_op(
        &mut self,
        left: &Expr,
        op: &BinOp,
        right: &Expr,
        span: &Span,
    ) -> Self::Output;
    fn gen_unary_op(&mut self, op: &UnOp, operand: &Expr, span: &Span) -> Self::Output;
    fn gen_block_expr(&mut self, block: &Block) -> Self::Output;
    fn gen_fn_call(&mut self, name: &str, args: &[Expr], span: &Span) -> Self::Output;
    fn gen_paren(&mut self, inner: &Expr, span: &Span) -> Self::Output;
}

/// One method per statement variant.
/// When every method is implemented, all possible statements are handled.
pub trait StmtGen {
    type Output;
    fn gen_let_stmt(
        &mut self,
        name: &str,
        is_mut: bool,
        type_: Option<&Type>,
        value: &Expr,
        span: &Span,
    ) -> Self::Output;
    fn gen_return_stmt(&mut self, expr: Option<&Expr>, span: &Span) -> Self::Output;
    fn gen_expr_stmt(&mut self, expr: &Expr, span: &Span) -> Self::Output;
    fn gen_if_stmt(
        &mut self,
        cond: &Expr,
        then_block: &Block,
        else_block: Option<&Block>,
        span: &Span,
    ) -> Self::Output;
}

/// One method per type variant.
pub trait TypeGen {
    type Output;
    fn gen_type(&mut self, type_: &Type) -> Self::Output;
}

/// Handles function definitions.
pub trait FnGen {
    type Output;
    fn gen_fn_def(&mut self, fn_def: &FnDef) -> Self::Output;
}

/// Orchestrates complete program generation by composing the other traits.
/// Implementors must also implement ExprGen + StmtGen + TypeGen + FnGen.
pub trait ProgramGen {
    type Output;
    type Error;
    fn gen_program(&mut self, prog: &Program) -> Result<Self::Output, Self::Error>;
}
