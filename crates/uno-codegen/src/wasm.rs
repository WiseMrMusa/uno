use crate::error::CodegenError;
use std::fmt::Write;
use uno_ir::{Inst, IrBackend, IrConstant, IrFunction, IrProgram, IrType, Value};

pub struct WasmCodegen {
    output: String,
    indent: usize,
    label_counter: usize,
    label_stack: Vec<String>,
}

impl WasmCodegen {
    pub fn new() -> Self {
        WasmCodegen {
            output: String::new(),
            indent: 0,
            label_counter: 0,
            label_stack: Vec::new(),
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

    fn val_str(&self, v: &Value) -> String {
        match v {
            Value::Local(id) => format!("(local.get ${id})"),
            Value::Const(c) => self.const_str(c),
        }
    }

    fn const_str(&self, c: &IrConstant) -> String {
        match c {
            IrConstant::Int(n) => n.to_string(),
            IrConstant::Bool(true) => "1".to_string(),
            IrConstant::Bool(false) => "0".to_string(),
        }
    }

    fn wat_type(ty: &IrType) -> Result<&str, CodegenError> {
        match ty {
            IrType::Bool => Ok("i32"),
            IrType::Uint(8) | IrType::Uint(16) | IrType::Uint(32) => Ok("i32"),
            IrType::Uint(64) => Ok("i64"),
            _ => Err(CodegenError::UnsupportedType(uno_syntax::ast::Type::Uint(
                if let IrType::Uint(n) = ty { *n } else { 0 },
            ))),
        }
    }

    fn wat_op(ty: &IrType, op: &str) -> Result<String, CodegenError> {
        let prefix = Self::wat_type(ty)?;
        Ok(format!("{prefix}.{op}"))
    }

    fn fresh_label(&mut self) -> usize {
        let id = self.label_counter;
        self.label_counter += 1;
        id
    }

    fn push_label(&mut self, label: String) {
        self.label_stack.push(label);
    }

    fn pop_label(&mut self) {
        self.label_stack.pop();
    }

    fn current_break_label(&self) -> String {
        self.label_stack.last().cloned().unwrap_or_default()
    }

    fn gen_insts(&mut self, insts: &[Inst]) -> Result<(), CodegenError> {
        for inst in insts {
            self.gen_inst(inst)?;
        }
        Ok(())
    }

    fn gen_inst(&mut self, inst: &Inst) -> Result<(), CodegenError> {
        match inst {
            Inst::Add(d, ty, a, b) => self.gen_binop(d, ty, a, b, "add"),
            Inst::Sub(d, ty, a, b) => self.gen_binop(d, ty, a, b, "sub"),
            Inst::Mul(d, ty, a, b) => self.gen_binop(d, ty, a, b, "mul"),
            Inst::Div(d, ty, a, b) => self.gen_binop(d, ty, a, b, "div_u"),
            Inst::Mod(d, ty, a, b) => self.gen_binop(d, ty, a, b, "rem_u"),
            Inst::Eq(d, a, b) => self.gen_cmp(d, a, b, "eq"),
            Inst::Neq(d, a, b) => self.gen_cmp(d, a, b, "ne"),
            Inst::Lt(d, a, b) => self.gen_cmp(d, a, b, "lt_u"),
            Inst::Gt(d, a, b) => self.gen_cmp(d, a, b, "gt_u"),
            Inst::Le(d, a, b) => self.gen_cmp(d, a, b, "le_u"),
            Inst::Ge(d, a, b) => self.gen_cmp(d, a, b, "ge_u"),
            Inst::And(d, a, b) => self.gen_logop(d, a, b, "and"),
            Inst::Or(d, a, b) => self.gen_logop(d, a, b, "or"),
            Inst::Not(d, a) => self.gen_unop(d, a, "eqz"),
            Inst::Neg(d, _ty, a) => {
                let av = self.val_str(a);
                self.writeln_indented(&format!(
                    "(local.set ${d} (i32.sub (i32.const 0) {av}))"
                ));
            }
            Inst::LoadConst(d, c, ty) => {
                let t = Self::wat_type(ty)?;
                let cv = self.const_str(c);
                self.writeln_indented(&format!("(local.set ${d} ({t}.const {cv}))"));
            }
            Inst::Load(d, _ty, src) => {
                self.writeln_indented(&format!("(local.set ${d} (local.get ${src}))"));
            }
            Inst::Store(dst, _ty, val) => {
                let v = self.val_str(val);
                self.writeln_indented(&format!("(local.set ${dst} {v})"));
            }
            Inst::Call(d, _ty, name, args) => {
                let a: Vec<_> = args.iter().map(|a| self.val_str(a)).collect();
                self.writeln_indented(&format!(
                    "(local.set ${d} (call ${} {}))",
                    name,
                    a.join(" ")
                ));
            }
            Inst::If(cond, then_, else_) => {
                let c = self.val_str(cond);
                if else_.is_empty() {
                    self.writeln_indented("(if");
                    self.push();
                    self.writeln_indented(&c);
                    self.writeln_indented("(then");
                    self.push();
                    self.gen_insts(then_)?;
                    self.pop();
                    self.writeln_indented(")");
                    self.pop();
                    self.writeln_indented(")");
                } else {
                    self.writeln_indented("(if");
                    self.push();
                    self.writeln_indented(&c);
                    self.writeln_indented("(then");
                    self.push();
                    self.gen_insts(then_)?;
                    self.pop();
                    self.writeln_indented(")");
                    self.writeln_indented("(else");
                    self.push();
                    self.gen_insts(else_)?;
                    self.pop();
                    self.writeln_indented(")");
                    self.pop();
                    self.writeln_indented(")");
                }
            }
            Inst::While(body) => {
                let brk = self.fresh_label();
                let loop_l = self.fresh_label();
                let brk_label = format!("while_break_{brk}");
                let loop_label = format!("while_loop_{loop_l}");
                self.push_label(brk_label.clone());
                self.writeln_indented(&format!("(block ${brk_label}"));
                self.push();
                self.writeln_indented(&format!("(loop ${loop_label}"));
                self.push();
                self.gen_insts(body)?;
                self.writeln_indented(&format!("(br ${loop_label})"));
                self.pop();
                self.writeln_indented(")");
                self.pop();
                self.writeln_indented(")");
                self.pop_label();
            }
            Inst::Loop(body) => {
                let brk = self.fresh_label();
                let loop_l = self.fresh_label();
                let brk_label = format!("loop_break_{brk}");
                let loop_label = format!("loop_loop_{loop_l}");
                self.push_label(brk_label.clone());
                self.writeln_indented(&format!("(block ${brk_label}"));
                self.push();
                self.writeln_indented(&format!("(loop ${loop_label}"));
                self.push();
                self.gen_insts(body)?;
                self.writeln_indented(&format!("(br ${loop_label})"));
                self.pop();
                self.writeln_indented(")");
                self.pop();
                self.writeln_indented(")");
                self.pop_label();
            }
            Inst::Break => {
                let brk = self.current_break_label();
                self.writeln_indented(&format!("(br ${brk})"));
            }
            Inst::Return(v) => match v {
                Some(val) => {
                    let vs = self.val_str(val);
                    self.writeln_indented(&vs);
                }
                None => self.writeln_indented("(return)"),
            },
            Inst::Drop(val) => {
                let v = self.val_str(val);
                self.writeln_indented(&format!("(drop {v})"));
            }
        }
        Ok(())
    }

    fn gen_binop(&mut self, d: &str, ty: &IrType, a: &Value, b: &Value, op: &str) {
        let instr = Self::wat_op(ty, op).unwrap_or_else(|_| format!("i32.{op}"));
        let av = self.val_str(a);
        let bv = self.val_str(b);
        self.writeln_indented(&format!("(local.set ${d} ({instr} {av} {bv}))"));
    }

    fn gen_cmp(&mut self, d: &str, a: &Value, b: &Value, op: &str) {
        let av = self.val_str(a);
        let bv = self.val_str(b);
        self.writeln_indented(&format!("(local.set ${d} (i32.{op} {av} {bv}))"));
    }

    fn gen_logop(&mut self, d: &str, a: &Value, b: &Value, op: &str) {
        let av = self.val_str(a);
        let bv = self.val_str(b);
        self.writeln_indented(&format!("(local.set ${d} (i32.{op} {av} {bv}))"));
    }

    fn gen_unop(&mut self, d: &str, a: &Value, op: &str) {
        let av = self.val_str(a);
        self.writeln_indented(&format!("(local.set ${d} (i32.{op} {av}))"));
    }

    fn gen_function(&mut self, func: &IrFunction) -> Result<(), CodegenError> {
        let ret = Self::wat_type(&func.return_type)?;

        let params: Vec<_> = func
            .params
            .iter()
            .map(|(n, ty)| {
                let t = Self::wat_type(ty).unwrap_or("i32");
                format!("(param ${n} {t})")
            })
            .collect();

        let locals: Vec<_> = func
            .locals
            .iter()
            .filter(|(n, _)| !func.params.iter().any(|(pn, _)| pn == n))
            .map(|(n, ty)| {
                let t = Self::wat_type(ty).unwrap_or("i32");
                format!("(local ${n} {t})")
            })
            .collect();

        let export = if func.name == "main" {
            " (export \"main\")"
        } else {
            ""
        };

        write!(
            self.output,
            "(func ${}{} {} (result {ret})\n",
            func.name,
            export,
            params.join(" ")
        )
        .unwrap();

        self.push();
        for l in &locals {
            self.writeln_indented(l);
        }
        if !locals.is_empty() {
            self.output.push('\n');
        }
        self.gen_insts(&func.insts)?;
        self.pop();
        self.writeln_indented(")");
        Ok(())
    }
}

impl IrBackend for WasmCodegen {
    type Output = String;
    type Error = CodegenError;

    fn name(&self) -> &str {
        "WASM codegen"
    }

    fn generate(&mut self, ir: &IrProgram) -> Result<String, CodegenError> {
        let mut out = String::from("(module\n");

        for func in &ir.functions {
            let mut cg = WasmCodegen::new();
            cg.indent = 1;
            cg.gen_function(func)?;
            out.push_str(&cg.output);
            out.push('\n');
        }

        out.push_str(")\n");
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uno_ir::lower::lower;

    fn wasm_generate(source: &str) -> Result<String, CodegenError> {
        let mut lexer = uno_lexer::Lexer::new(source.to_string());
        let tokens = lexer.tokenize();
        let mut parser = uno_parser::Parser::new(tokens);
        let program = parser.parse_program().unwrap();
        let ir = lower(&program).unwrap();
        WasmCodegen::new().generate(&ir)
    }

    #[test]
    fn simple_function() {
        let out = wasm_generate("fn main() -> u32 { return 42; }").unwrap();
        assert!(out.contains("(module"));
        assert!(out.contains("(func $main"));
        assert!(out.contains("(export \"main\")"));
        assert!(out.contains("i32.const 42"));
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
    }

    #[test]
    fn while_loop_output() {
        let src = "fn main() -> u32 { while true { return 1; } return 0; }";
        let out = wasm_generate(src).unwrap();
        assert!(out.contains("(block $while_break"));
        assert!(out.contains("(loop $while_loop"));
        assert!(out.contains("(br $while_break"));
    }

    #[test]
    fn loop_break_output() {
        let src = "fn main() -> u32 { loop { break; } return 0; }";
        let out = wasm_generate(src).unwrap();
        assert!(out.contains("(block $loop_break"));
        assert!(out.contains("(loop $loop_loop"));
        assert!(out.contains("(br $loop_break"));
    }
}
