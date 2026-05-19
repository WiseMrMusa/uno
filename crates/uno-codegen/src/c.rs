use crate::error::CodegenError;
use std::fmt::Write;
use uno_ir::{Inst, IrBackend, IrConstant, IrFunction, IrProgram, IrType, Value};

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

    fn val_str(&self, v: &Value) -> String {
        match v {
            Value::Local(id) => id.clone(),
            Value::Const(c) => self.const_str(c),
        }
    }

    fn const_str(&self, c: &IrConstant) -> String {
        match c {
            IrConstant::Int(n) => n.to_string(),
            IrConstant::Bool(true) => "true".to_string(),
            IrConstant::Bool(false) => "false".to_string(),
        }
    }

    fn c_type(ty: &IrType) -> Result<&str, CodegenError> {
        match ty {
            IrType::Bool => Ok("bool"),
            IrType::Uint(8) => Ok("uint8_t"),
            IrType::Uint(16) => Ok("uint16_t"),
            IrType::Uint(32) => Ok("uint32_t"),
            IrType::Uint(64) => Ok("uint64_t"),
            IrType::Uint(128) => Ok("unsigned __int128"),
            _ => Err(CodegenError::UnsupportedType(uno_syntax::ast::Type::Uint(
                if let IrType::Uint(n) = ty { *n } else { 0 },
            ))),
        }
    }

    fn gen_inst(&mut self, inst: &Inst) -> Result<(), CodegenError> {
        match inst {
            Inst::Add(d, ty, a, b) => self.gen_binop(d, ty, a, b, "+"),
            Inst::Sub(d, ty, a, b) => self.gen_binop(d, ty, a, b, "-"),
            Inst::Mul(d, ty, a, b) => self.gen_binop(d, ty, a, b, "*"),
            Inst::Div(d, ty, a, b) => self.gen_binop(d, ty, a, b, "/"),
            Inst::Mod(d, ty, a, b) => self.gen_binop(d, ty, a, b, "%"),
            Inst::Eq(d, a, b) => self.gen_cmp(d, a, b, "=="),
            Inst::Neq(d, a, b) => self.gen_cmp(d, a, b, "!="),
            Inst::Lt(d, a, b) => self.gen_cmp(d, a, b, "<"),
            Inst::Gt(d, a, b) => self.gen_cmp(d, a, b, ">"),
            Inst::Le(d, a, b) => self.gen_cmp(d, a, b, "<="),
            Inst::Ge(d, a, b) => self.gen_cmp(d, a, b, ">="),
            Inst::And(d, a, b) => self.gen_binop(d, &IrType::Bool, a, b, "&&"),
            Inst::Or(d, a, b) => self.gen_binop(d, &IrType::Bool, a, b, "||"),
            Inst::Not(d, a) => self.gen_unop(d, a, "!"),
            Inst::Neg(d, ty, a) => {
                let t = Self::c_type(ty)?;
                let av = self.val_str(a);
                self.writeln_indented(&format!("{t} {d} = -({av});"));
            }
            Inst::LoadConst(d, c, ty) => {
                let t = Self::c_type(ty)?;
                let cv = self.const_str(c);
                self.writeln_indented(&format!("{t} {d} = {cv};"));
            }
            Inst::Load(d, ty, src) => {
                let t = Self::c_type(ty)?;
                self.writeln_indented(&format!("{t} {d} = {src};"));
            }
            Inst::Store(dst, _ty, val) => {
                let v = self.val_str(val);
                self.writeln_indented(&format!("{dst} = {v};"));
            }
            Inst::Call(d, ty, name, args) => {
                let t = Self::c_type(ty)?;
                let a: Vec<_> = args.iter().map(|a| self.val_str(a)).collect();
                self.writeln_indented(&format!("{t} {d} = {}({});", name, a.join(", ")));
            }
            Inst::If(cond, then_, else_) => {
                let c = self.val_str(cond);
                self.writeln_indented(&format!("if ({c}) {{"));
                self.push();
                for i in then_ { self.gen_inst(i)?; }
                self.pop();
                if else_.is_empty() {
                    self.writeln_indented("}");
                } else {
                    self.writeln_indented("} else {");
                    self.push();
                    for i in else_ { self.gen_inst(i)?; }
                    self.pop();
                    self.writeln_indented("}");
                }
            }
            Inst::While(body) => {
                self.writeln_indented("while (1) {");
                self.push();
                for i in body { self.gen_inst(i)?; }
                self.pop();
                self.writeln_indented("}");
            }
            Inst::Loop(body) => {
                self.writeln_indented("while (1) {");
                self.push();
                for i in body { self.gen_inst(i)?; }
                self.pop();
                self.writeln_indented("}");
            }
            Inst::Break => {
                self.writeln_indented("break;");
            }
            Inst::Return(v) => match v {
                Some(val) => self.writeln_indented(&format!("return {};", self.val_str(val))),
                None => self.writeln_indented("return;"),
            },
            Inst::Drop(val) => {
                let v = self.val_str(val);
                self.writeln_indented(&format!("(void){v};"));
            }
        }
        Ok(())
    }

    fn gen_binop(&mut self, d: &str, ty: &IrType, a: &Value, b: &Value, op: &str) {
        let t = Self::c_type(ty).unwrap_or("auto");
        let av = self.val_str(a);
        let bv = self.val_str(b);
        self.writeln_indented(&format!("{t} {d} = ({t})({av}) {op} ({bv});"));
    }

    fn gen_cmp(&mut self, d: &str, a: &Value, b: &Value, op: &str) {
        let av = self.val_str(a);
        let bv = self.val_str(b);
        self.writeln_indented(&format!("bool {d} = ({av}) {op} ({bv});"));
    }

    fn gen_unop(&mut self, d: &str, a: &Value, op: &str) {
        let av = self.val_str(a);
        self.writeln_indented(&format!("bool {d} = {op}{av};"));
    }

    fn gen_function(&mut self, func: &IrFunction) -> Result<(), CodegenError> {
        let ret = Self::c_type(&func.return_type)?;
        if func.name == "main" {
            self.output.push_str("int main(int argc, char** argv)");
        } else {
            write!(self.output, "{ret} {}", func.name).unwrap();
            self.output.push('(');
            for (i, (name, ty)) in func.params.iter().enumerate() {
                if i > 0 { self.output.push_str(", "); }
                let t = Self::c_type(ty)?;
                write!(self.output, "{t} {name}").unwrap();
            }
            self.output.push(')');
        }
        self.output.push_str(" {\n");
        self.push();
        for (name, ty) in &func.locals {
            if !name.starts_with("_t") && !func.params.iter().any(|(pn, _)| pn == name) {
                let t = Self::c_type(ty)?;
                self.writeln_indented(&format!("{t} {name};"));
            }
        }
        for inst in &func.insts {
            self.gen_inst(inst)?;
        }
        self.pop();
        self.writeln_indented("}");
        Ok(())
    }
}

impl IrBackend for Codegen {
    type Output = String;
    type Error = CodegenError;

    fn name(&self) -> &str {
        "C codegen"
    }

    fn generate(&mut self, ir: &IrProgram) -> Result<String, CodegenError> {
        let mut out = String::new();
        out.push_str("// Generated by Uno compiler\n");
        out.push_str("#include <stdint.h>\n#include <stdbool.h>\n#include <stdio.h>\n\n");

        for func in &ir.functions {
            let ret = Self::c_type(&func.return_type)?;
            if func.name == "main" {
                out.push_str("int main(int argc, char** argv);\n");
            } else {
                out.push_str(ret);
                out.push(' ');
                out.push_str(&func.name);
                out.push('(');
                for (i, (name, ty)) in func.params.iter().enumerate() {
                    if i > 0 { out.push_str(", "); }
                    let t = Self::c_type(ty)?;
                    out.push_str(t);
                    out.push(' ');
                    out.push_str(name);
                }
                out.push_str(");\n");
            }
        }
        out.push('\n');

        for func in &ir.functions {
            let mut cg = Codegen::new();
            cg.gen_function(func)?;
            out.push_str(&cg.output);
            out.push('\n');
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uno_ir::lower::lower;

    fn c_generate(source: &str) -> Result<String, CodegenError> {
        let mut lexer = uno_lexer::Lexer::new(source.to_string());
        let tokens = lexer.tokenize();
        let mut parser = uno_parser::Parser::new(tokens);
        let program = parser.parse_program().unwrap();
        let ir = lower(&program).unwrap();
        Codegen::new().generate(&ir)
    }

    #[test]
    fn empty_main() {
        let out = c_generate("fn main() -> u32 { return 0; }").unwrap();
        assert!(out.contains("int main"));
        assert!(out.contains("return"));
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
    }

    #[test]
    fn unsupported_type_returns_error() {
        let src = "fn main() -> u256 { return 0; }";
        let result = c_generate(src);
        assert!(result.is_err());
    }

    #[test]
    fn while_loop_output() {
        let out = c_generate("fn main() -> u32 { while true { return 1; } return 0; }").unwrap();
        assert!(out.contains("while (1)"));
        assert!(out.contains("return"));
    }

    #[test]
    fn loop_break_output() {
        let out = c_generate("fn main() -> u32 { loop { break; } return 0; }").unwrap();
        assert!(out.contains("while (1)"));
        assert!(out.contains("break;"));
    }

    #[test]
    fn assignment_output() {
        let src = "fn main() -> u32 { let mut x: u32 = 0; x = 5; return x; }";
        let out = c_generate(src).unwrap();
        assert!(out.contains("uint32_t x;"));
        assert!(out.contains("x = "));
    }
}
