use std::collections::HashSet;
use std::fmt::Write;
use uno_syntax::ast::*;
use crate::error::CodegenError;
use crate::generate::{ExprGen, FnGen, ProgramGen, StmtGen, TypeGen};
use uno_syntax::backend::Backend;
use uno_syntax::span::Span;

pub struct WasmCodegen {
    output: String,
    indent: usize,
    locals: HashSet<String>,
    local_types: Vec<(String, String)>,
    fn_result: Option<&'static str>,
}

impl WasmCodegen {
    pub fn new() -> Self {
        WasmCodegen {
            output: String::new(),
            indent: 0,
            locals: HashSet::new(),
            local_types: Vec::new(),
            fn_result: None,
        }
    }

    fn type_to_wat(type_: &Type) -> Result<&'static str, CodegenError> {
        match type_ {
            Type::Bool => Ok("i32"),
            Type::Uint(8) | Type::Uint(16) | Type::Uint(32) => Ok("i32"),
            Type::Uint(64) => Ok("i64"),
            _ => Err(CodegenError::UnsupportedType(type_.clone())),
        }
    }

    fn indent_str() -> &'static str {
        "  "
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

    fn determine_wat_type(&self, expr: &Expr) -> &'static str {
        if self.is_i64_expr(expr) {
            "i64"
        } else {
            "i32"
        }
    }

    fn is_i64_expr(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Ident(name, _) => self
                .local_types
                .iter()
                .any(|(n, t)| n == name && t == "i64"),
            Expr::Literal(val, _) => {
                let clean: String = val.chars().take_while(|c| c.is_ascii_digit()).collect();
                clean.parse::<u64>().map_or(false, |n| n > u32::MAX as u64)
                    || val.contains("u64") || val.contains("u128")
            }
            Expr::BinaryOp(left, _, right, _) => self.is_i64_expr(left) || self.is_i64_expr(right),
            Expr::UnaryOp(_, operand, _) => self.is_i64_expr(operand),
            Expr::FnCall(name, _, _) => self
                .local_types
                .iter()
                .any(|(n, t)| n == name && t == "i64"),
            _ => false,
        }
    }

    fn parse_literal(val: &str) -> Result<(String, bool), CodegenError> {
        let hex = val.starts_with("0x") || val.starts_with("0X");
        let clean: String = if hex {
            val[2..].chars().take_while(|c| c.is_ascii_hexdigit()).collect()
        } else {
            val.chars().take_while(|c| c.is_ascii_digit()).collect()
        };
        let is_64 = if hex {
            u64::from_str_radix(&clean, 16).map_or(false, |n| n > u32::MAX as u64)
        } else {
            let numeric = if clean.is_empty() { "0" } else { &clean };
            numeric.parse::<u64>().map_or(false, |n| n > u32::MAX as u64)
                || val.contains("u64") || val.contains("u128")
        };
        let prefix = if hex { "0x" } else { "" };
        Ok((format!("{}{}", prefix, clean), is_64))
    }

    fn collect_locals_from_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            match stmt {
                Stmt::Let(name, _, type_, _, _) => {
                    if !self.locals.contains(name) {
                        self.locals.insert(name.clone());
                        let wat_type = match type_ {
                            Some(t) => Self::type_to_wat(t).unwrap_or("i32").to_string(),
                            None => "i32".to_string(),
                        };
                        self.local_types.push((name.clone(), wat_type));
                    }
                }
                Stmt::If(_, then_block, else_block, _) => {
                    self.collect_locals_from_block(then_block);
                    if let Some(b) = else_block {
                        self.collect_locals_from_block(b);
                    }
                }
                _ => {}
            }
        }
    }

    fn gen_block_body(&mut self, block: &Block) -> Result<(), CodegenError> {
        for stmt in &block.stmts {
            self.write_indent();
            self.gen_stmt(stmt)?;
        }
        Ok(())
    }

    fn gen_stmt(&mut self, stmt: &Stmt) -> Result<(), CodegenError> {
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
        let mut cg = WasmCodegen::new();
        cg.gen_program(prog)
    }
}

impl TypeGen for WasmCodegen {
    type Output = String;
    type Error = CodegenError;

    fn gen_type(&mut self, type_: &Type) -> Result<String, CodegenError> {
        Self::type_to_wat(type_).map(|s| s.to_string())
    }
}

impl ExprGen for WasmCodegen {
    type Output = String;
    type Error = CodegenError;

    fn gen_literal(&mut self, val: &str, _span: &Span) -> Result<String, CodegenError> {
        let (num, is_64) = Self::parse_literal(val)?;
        if is_64 {
            Ok(format!("(i64.const {})", num))
        } else {
            Ok(format!("(i32.const {})", num))
        }
    }

    fn gen_ident(&mut self, name: &str, _span: &Span) -> Result<String, CodegenError> {
        match name {
            "true" => Ok("(i32.const 1)".to_string()),
            "false" => Ok("(i32.const 0)".to_string()),
            _ => Ok(format!("(local.get ${})", name)),
        }
    }

    fn gen_binary_op(
        &mut self,
        left: &Expr,
        op: &BinOp,
        right: &Expr,
        _span: &Span,
    ) -> Result<String, CodegenError> {
        let i = self.determine_wat_type(left);
        let l = self.gen_expr(left)?;
        let r = self.gen_expr(right)?;

        let instr = match op {
            BinOp::Add => format!("{i}.add"),
            BinOp::Sub => format!("{i}.sub"),
            BinOp::Mul => format!("{i}.mul"),
            BinOp::Div => format!("{i}.div_u"),
            BinOp::Mod => format!("{i}.rem_u"),
            BinOp::Eq => "i32.eq".to_string(),
            BinOp::Neq => "i32.ne".to_string(),
            BinOp::Lt => format!("{i}.lt_u"),
            BinOp::Gt => format!("{i}.gt_u"),
            BinOp::Le => format!("{i}.le_u"),
            BinOp::Ge => format!("{i}.ge_u"),
            BinOp::And => "i32.and".to_string(),
            BinOp::Or => "i32.or".to_string(),
        };

        Ok(format!("({instr} {l} {r})"))
    }

    fn gen_unary_op(
        &mut self,
        op: &UnOp,
        operand: &Expr,
        _span: &Span,
    ) -> Result<String, CodegenError> {
        let inner = self.gen_expr(operand)?;
        match op {
            UnOp::Neg => Ok(format!("(i32.sub (i32.const 0) {inner})")),
            UnOp::Not => Ok(format!("(i32.eqz {inner})")),
        }
    }

    fn gen_block_expr(&mut self, block: &Block) -> Result<String, CodegenError> {
        let mut cg = WasmCodegen::new();
        cg.indent = self.indent;
        cg.locals = self.locals.clone();
        cg.local_types = self.local_types.clone();
        cg.fn_result = self.fn_result;
        cg.output.push_str("(block\n");
        cg.push();
        for stmt in &block.stmts {
            cg.write_indent();
            cg.gen_stmt(stmt)?;
        }
        cg.pop();
        cg.write_indent();
        cg.output.push_str(")");
        self.locals = cg.locals;
        self.local_types = cg.local_types;
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
        Ok(format!("(call ${} {})", name, args_str.join(" ")))
    }

    fn gen_paren(&mut self, inner: &Expr, _span: &Span) -> Result<String, CodegenError> {
        self.gen_expr(inner)
    }
}

impl StmtGen for WasmCodegen {
    type Output = ();
    type Error = CodegenError;

    fn gen_let_stmt(
        &mut self,
        name: &str,
        _is_mut: bool,
        _type_: Option<&Type>,
        value: &Expr,
        _span: &Span,
    ) -> Result<(), CodegenError> {
        let val = self.gen_expr(value)?;
        write!(self.output, "(local.set ${name} {val})\n").unwrap();
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
                write!(self.output, "{val}\n").unwrap();
            }
            None => self.output.push_str("(return)\n"),
        }
        Ok(())
    }

    fn gen_expr_stmt(&mut self, expr: &Expr, _span: &Span) -> Result<(), CodegenError> {
        let val = self.gen_expr(expr)?;
        write!(self.output, "(drop {val})\n").unwrap();
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
        match self.fn_result {
            Some(ty) => write!(self.output, "(if (result {ty})").unwrap(),
            None => self.output.push_str("(if"),
        }
        self.output.push('\n');
        self.push();
        self.writeln_indented(&cond_str);
        self.writeln_indented("(then");
        self.push();
        for stmt in &then_block.stmts {
            self.write_indent();
            self.gen_stmt(stmt)?;
        }
        self.pop();
        self.writeln_indented(")");

        if let Some(block) = else_block {
            self.writeln_indented("(else");
            self.push();
            for stmt in &block.stmts {
                self.write_indent();
                self.gen_stmt(stmt)?;
            }
            self.pop();
            self.writeln_indented(")");
        }

        self.pop();
        self.writeln_indented(")");
        Ok(())
    }
}

impl FnGen for WasmCodegen {
    type Output = ();
    type Error = CodegenError;

    fn gen_fn_def(&mut self, fn_def: &FnDef) -> Result<(), CodegenError> {
        self.locals.clear();
        self.local_types.clear();
        let ret_wat = Self::type_to_wat(&fn_def.return_type)?;
        self.fn_result = Some(ret_wat);

        let params_str: Vec<String> = fn_def
            .params
            .iter()
            .map(|p| {
                let t = Self::type_to_wat(&p.type_).unwrap_or("i32");
                self.locals.insert(p.name.clone());
                self.local_types.push((p.name.clone(), t.to_string()));
                format!("(param ${} {})", p.name, t)
            })
            .collect();

        self.collect_locals_from_block(&fn_def.body);

        let local_decls: Vec<String> = self
            .local_types
            .iter()
            .filter(|(n, _)| !fn_def.params.iter().any(|p| p.name == *n))
            .map(|(_, t)| format!("(local {t})"))
            .collect();

        let export = if fn_def.name == "main" {
            " (export \"main\")"
        } else {
            ""
        };

        write!(
            self.output,
            "(func ${}{} {} (result {ret_wat})\n",
            fn_def.name,
            export,
            params_str.join(" ")
        )
        .unwrap();

        self.push();
        for decl in &local_decls {
            self.writeln_indented(&decl);
        }
        if !local_decls.is_empty() {
            self.output.push('\n');
        }
        self.gen_block_body(&fn_def.body)?;
        self.pop();
        self.writeln_indented(")");
        Ok(())
    }
}

impl ProgramGen for WasmCodegen {
    type Output = String;
    type Error = CodegenError;

    fn gen_program(&mut self, prog: &Program) -> Result<String, CodegenError> {
        let mut out = String::from("(module\n");

        for fn_def in &prog.functions {
            let mut cg = WasmCodegen::new();
            cg.indent = 1;
            cg.gen_fn_def(fn_def)?;
            out.push_str(&cg.output);
            out.push('\n');
        }

        out.push_str(")\n");
        Ok(out)
    }
}

impl Backend for WasmCodegen {
    type Output = String;
    type Err = CodegenError;

    fn name(&self) -> &str {
        "WASM codegen"
    }

    fn generate(&mut self, prog: &Program) -> Result<String, CodegenError> {
        self.gen_program(prog)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wasm_generate(source: &str) -> Result<String, CodegenError> {
        let mut lexer = uno_lexer::Lexer::new(source.to_string());
        let tokens = lexer.tokenize();
        let mut parser = uno_parser::Parser::new(tokens);
        let program = parser.parse_program().unwrap();
        WasmCodegen::generate(&program)
    }

    #[test]
    fn simple_function() {
        let out = wasm_generate("fn main() -> u32 { return 42; }").unwrap();
        assert!(out.contains("(module"));
        assert!(out.contains("(func $main"));
        assert!(out.contains("(export \"main\")"));
        assert!(out.contains("(i32.const 42)"));
        assert!(out.contains(")"));
    }

    #[test]
    fn fib_sequence() {
        let src = "fn fib(n: u32) -> u32 { if n <= 1 { return n; } return fib(n - 1) + fib(n - 2); }
                    fn main() -> u32 { return fib(10); }";
        let out = wasm_generate(src).unwrap();
        assert!(out.contains("(func $fib"));
        assert!(out.contains("(func $main"));
    }

    #[test]
    fn unsupported_type_returns_error() {
        let src = "fn main() -> u256 { return 0; }";
        let result = wasm_generate(src);
        assert!(result.is_err());
        assert!(matches!(result, Err(CodegenError::UnsupportedType(Type::Uint(256)))));
    }
}
