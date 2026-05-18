use crate::error::CodegenError;
use crate::generate::{ExprGen, FnGen, ProgramGen, StmtGen, TypeGen};
use std::fmt::Write;
use uno_syntax::ast::*;
use uno_syntax::backend::Backend;
use uno_syntax::span::Span;

pub struct Codegen {
    output: String,
    indent: usize,
}

impl Codegen {
    pub fn new() -> Self {
        Codegen {
            output: String::new(),
            indent: 0,
        }
    }

    fn indent_str() -> &'static str {
        "    "
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str(Self::indent_str());
        }
    }

    fn push(&mut self) {
        self.indent += 1;
    }

    fn pop(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    fn writeln_indented(&mut self, s: &str) {
        self.write_indent();
        self.output.push_str(s);
        self.output.push('\n');
    }

    fn gen_block_body(&mut self, block: &Block) -> Result<(), CodegenError> {
        self.push();
        for stmt in &block.stmts {
            self.gen_stmt(stmt)?;
        }
        self.pop();
        Ok(())
    }

    fn gen_stmt(&mut self, stmt: &Stmt) -> Result<(), CodegenError> {
        self.write_indent();
        match stmt {
            Stmt::Let(name, is_mut, type_, value, span) => {
                self.gen_let_stmt(name, *is_mut, type_.as_ref(), value, span)
            }
            Stmt::Return(expr, span) => self.gen_return_stmt(expr.as_ref(), span),
            Stmt::Expr(expr, span) => self.gen_expr_stmt(expr, span),
            Stmt::If(cond, then_block, else_block, span) => {
                self.gen_if_stmt(cond, then_block, else_block.as_ref(), span)
            }
        }
    }

    fn gen_expr(&mut self, expr: &Expr) -> Result<String, CodegenError> {
        match expr {
            Expr::Literal(val, span) => self.gen_literal(val, span),
            Expr::Ident(name, span) => self.gen_ident(name, span),
            Expr::BinaryOp(left, op, right, span) => {
                self.gen_binary_op(left, op, right, span)
            }
            Expr::UnaryOp(op, operand, span) => self.gen_unary_op(op, operand, span),
            Expr::Block(block, _) => self.gen_block_expr(block),
            Expr::FnCall(name, args, span) => self.gen_fn_call(name, args, span),
            Expr::Paren(inner, span) => self.gen_paren(inner, span),
        }
    }

    pub fn generate(prog: &Program) -> Result<String, CodegenError> {
        let mut cg = Codegen::new();
        cg.gen_program(prog)
    }
}

impl TypeGen for Codegen {
    type Output = String;
    type Error = CodegenError;

    fn gen_type(&mut self, type_: &Type) -> Result<String, CodegenError> {
        match type_ {
            Type::Bool => Ok("bool".to_string()),
            Type::Uint(8) => Ok("uint8_t".to_string()),
            Type::Uint(16) => Ok("uint16_t".to_string()),
            Type::Uint(32) => Ok("uint32_t".to_string()),
            Type::Uint(64) => Ok("uint64_t".to_string()),
            Type::Uint(128) => Ok("unsigned __int128".to_string()),
            _ => Err(CodegenError::UnsupportedType(type_.clone())),
        }
    }
}

impl ExprGen for Codegen {
    type Output = String;
    type Error = CodegenError;

    fn gen_literal(&mut self, val: &str, _span: &Span) -> Result<String, CodegenError> {
        Ok(val.to_string())
    }

    fn gen_ident(&mut self, name: &str, _span: &Span) -> Result<String, CodegenError> {
        match name {
            "true" => Ok("true".to_string()),
            "false" => Ok("false".to_string()),
            _ => Ok(name.to_string()),
        }
    }

    fn gen_binary_op(
        &mut self,
        left: &Expr,
        op: &BinOp,
        right: &Expr,
        _span: &Span,
    ) -> Result<String, CodegenError> {
        let op_str = match op {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Eq => "==",
            BinOp::Neq => "!=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
        };
        Ok(format!(
            "({} {} {})",
            self.gen_expr(left)?,
            op_str,
            self.gen_expr(right)?
        ))
    }

    fn gen_unary_op(
        &mut self,
        op: &UnOp,
        operand: &Expr,
        _span: &Span,
    ) -> Result<String, CodegenError> {
        let op_str = match op {
            UnOp::Neg => "-",
            UnOp::Not => "!",
        };
        Ok(format!("{}{}", op_str, self.gen_expr(operand)?))
    }

    fn gen_block_expr(&mut self, block: &Block) -> Result<String, CodegenError> {
        let outer_indent = self.indent;
        let mut cg = Codegen::new();
        cg.indent = outer_indent;
        cg.output.push_str("{\n");
        cg.indent = outer_indent + 1;
        for stmt in &block.stmts {
            cg.gen_stmt(stmt)?;
        }
        cg.indent = outer_indent;
        cg.writeln_indented("}");
        Ok(cg.output)
    }

    fn gen_fn_call(
        &mut self,
        name: &str,
        args: &[Expr],
        _span: &Span,
    ) -> Result<String, CodegenError> {
        let args_str: Vec<String> = args
            .iter()
            .map(|a| self.gen_expr(a))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(format!("{}({})", name, args_str.join(", ")))
    }

    fn gen_paren(&mut self, inner: &Expr, _span: &Span) -> Result<String, CodegenError> {
        Ok(format!("({})", self.gen_expr(inner)?))
    }
}

impl StmtGen for Codegen {
    type Output = ();
    type Error = CodegenError;

    fn gen_let_stmt(
        &mut self,
        name: &str,
        _is_mut: bool,
        type_: Option<&Type>,
        value: &Expr,
        _span: &Span,
    ) -> Result<(), CodegenError> {
        let type_str = match type_ {
            Some(t) => self.gen_type(t)?,
            None => "auto".to_string(),
        };
        let val = self.gen_expr(value)?;
        self.output.push_str(&type_str);
        self.output.push(' ');
        self.output.push_str(name);
        self.output.push_str(" = ");
        self.output.push_str(&val);
        self.output.push_str(";\n");
        Ok(())
    }

    fn gen_return_stmt(
        &mut self,
        expr: Option<&Expr>,
        _span: &Span,
    ) -> Result<(), CodegenError> {
        match expr {
            Some(e) => {
                let val = self.gen_expr(e)?;
                self.output.push_str("return ");
                self.output.push_str(&val);
                self.output.push_str(";\n");
            }
            None => self.output.push_str("return;\n"),
        }
        Ok(())
    }

    fn gen_expr_stmt(&mut self, expr: &Expr, _span: &Span) -> Result<(), CodegenError> {
        let val = self.gen_expr(expr)?;
        self.output.push_str(&val);
        self.output.push_str(";\n");
        Ok(())
    }

    fn gen_if_stmt(
        &mut self,
        cond: &Expr,
        then_block: &Block,
        else_block: Option<&Block>,
        _span: &Span,
    ) -> Result<(), CodegenError> {
        let cond_str = self.gen_expr(cond)?;
        self.output.push_str("if (");
        self.output.push_str(&cond_str);
        self.output.push_str(") {\n");
        self.push();
        for stmt in &then_block.stmts {
            self.gen_stmt(stmt)?;
        }
        self.pop();
        match else_block {
            Some(block)
                if block.stmts.len() == 1 && matches!(block.stmts.first(), Some(Stmt::If(..))) =>
            {
                self.write_indent();
                self.output.push_str("} else ");
                self.gen_stmt(&block.stmts[0])?;
            }
            Some(block) => {
                self.write_indent();
                self.output.push_str("} else {\n");
                self.push();
                for stmt in &block.stmts {
                    self.gen_stmt(stmt)?;
                }
                self.pop();
                self.write_indent();
                self.output.push_str("}\n");
            }
            None => {
                self.write_indent();
                self.output.push_str("}\n");
            }
        }
        Ok(())
    }
}

impl FnGen for Codegen {
    type Output = ();
    type Error = CodegenError;

    fn gen_fn_def(&mut self, fn_def: &FnDef) -> Result<(), CodegenError> {
        let ret_type = self.gen_type(&fn_def.return_type)?;

        if fn_def.name == "main" {
            self.output.push_str("int main(int argc, char** argv)");
        } else {
            write!(self.output, "{} {}(", ret_type, fn_def.name).unwrap();
            for (i, param) in fn_def.params.iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                let pt = self.gen_type(&param.type_)?;
                write!(self.output, "{} {}", pt, param.name).unwrap();
            }
            self.output.push(')');
        }

        self.output.push_str(" {\n");
        self.gen_block_body(&fn_def.body)?;
        self.writeln_indented("}");
        Ok(())
    }
}

impl ProgramGen for Codegen {
    type Output = String;
    type Error = CodegenError;

    fn gen_program(&mut self, prog: &Program) -> Result<String, CodegenError> {
        let mut out = String::new();

        out.push_str("// Generated by Uno compiler\n");
        out.push_str("#include <stdint.h>\n");
        out.push_str("#include <stdbool.h>\n");
        out.push_str("#include <stdio.h>\n\n");

        for fn_def in &prog.functions {
            let ret_type = self.gen_type(&fn_def.return_type)?;
            if fn_def.name == "main" {
                out.push_str("int main(int argc, char** argv);\n");
            } else {
                out.push_str(&ret_type);
                out.push(' ');
                out.push_str(&fn_def.name);
                out.push('(');
                for (i, param) in fn_def.params.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    let pt = self.gen_type(&param.type_)?;
                    out.push_str(&pt);
                    out.push(' ');
                    out.push_str(&param.name);
                }
                out.push_str(");\n");
            }
        }
        out.push('\n');

        for fn_def in &prog.functions {
            let mut cg = Codegen::new();
            cg.gen_fn_def(fn_def)?;
            out.push_str(&cg.output);
            out.push('\n');
        }

        Ok(out)
    }
}

impl Backend for Codegen {
    type Output = String;
    type Err = CodegenError;

    fn name(&self) -> &str {
        "C codegen"
    }

    fn generate(&mut self, prog: &Program) -> Result<String, CodegenError> {
        self.gen_program(prog)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c_generate(source: &str) -> Result<String, CodegenError> {
        let mut lexer = uno_lexer::Lexer::new(source.to_string());
        let tokens = lexer.tokenize();
        let mut parser = uno_parser::Parser::new(tokens);
        let program = parser.parse_program().unwrap();
        Codegen::generate(&program)
    }

    #[test]
    fn empty_main() {
        let out = c_generate("fn main() -> u32 { return 0; }").unwrap();
        assert!(out.contains("int main"));
        assert!(out.contains("return 0;"));
    }

    #[test]
    fn fib_sequence() {
        let src = "fn fib(n: u32) -> u32 { if n <= 1 { return n; } return fib(n - 1) + fib(n - 2); }
                    fn main() -> u32 { return fib(10); }";
        let out = c_generate(src).unwrap();
        assert!(out.contains("uint32_t fib(uint32_t n)"));
        assert!(out.contains("int main"));
        assert!(out.contains("fib("));
    }

    #[test]
    fn includes_present() {
        let out = c_generate("fn main() -> u32 { return 0; }").unwrap();
        assert!(out.contains("#include <stdint.h>"));
        assert!(out.contains("#include <stdbool.h>"));
        assert!(out.contains("#include <stdio.h>"));
    }

    #[test]
    fn unsupported_type_returns_error() {
        let src = "fn main() -> u256 { return 0; }";
        let result = c_generate(src);
        assert!(result.is_err());
        assert!(matches!(result, Err(CodegenError::UnsupportedType(Type::Uint(256)))));
    }
}
