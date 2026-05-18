use std::fmt::Write;
use uno_syntax::ast::*;
use uno_syntax::backend::Backend;

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

    fn indent_str(&self) -> &'static str {
        "    "
    }

    fn push(&mut self) {
        self.indent += 1;
    }

    fn pop(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    fn write(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.output.push_str(self.indent_str());
        }
        self.output.push_str(s);
    }

    fn writeln(&mut self, s: &str) {
        self.write(s);
        self.output.push('\n');
    }

    fn type_to_c(type_: &Type) -> &'static str {
        match type_ {
            Type::Bool => "bool",
            Type::Uint(8) => "uint8_t",
            Type::Uint(16) => "uint16_t",
            Type::Uint(32) => "uint32_t",
            Type::Uint(64) => "uint64_t",
            Type::Uint(128) => "unsigned __int128",
            Type::Uint(bits) => panic!("unsupported uint size: u{bits}"),
        }
    }

    fn binop_to_c(op: &BinOp) -> &'static str {
        match op {
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
        }
    }

    fn unop_to_c(op: &UnOp) -> &'static str {
        match op {
            UnOp::Neg => "-",
            UnOp::Not => "!",
        }
    }

    fn gen_expr(expr: &Expr, out: &mut String) {
        match expr {
            Expr::Literal(val, _) => out.push_str(val),
            Expr::Ident(name, _) => {
                if name == "true" {
                    out.push_str("true");
                } else if name == "false" {
                    out.push_str("false");
                } else {
                    out.push_str(name);
                }
            }
            Expr::BinaryOp(left, op, right, _) => {
                out.push('(');
                Self::gen_expr(left, out);
                out.push(' ');
                out.push_str(Self::binop_to_c(op));
                out.push(' ');
                Self::gen_expr(right, out);
                out.push(')');
            }
            Expr::UnaryOp(op, operand, _) => {
                out.push_str(Self::unop_to_c(op));
                Self::gen_expr(operand, out);
            }
            Expr::Block(block, _) => {
                Self::gen_block(block, out);
            }
            Expr::FnCall(name, args, _) => {
                out.push_str(name);
                out.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    Self::gen_expr(arg, out);
                }
                out.push(')');
            }
            Expr::Paren(inner, _) => {
                out.push('(');
                Self::gen_expr(inner, out);
                out.push(')');
            }
        }
    }

    fn gen_block(block: &Block, out: &mut String) {
        out.push_str("{\n");
        let indent = out.len();
        let _ = indent;
        for stmt in &block.stmts {
            Self::gen_stmt(stmt, out);
        }
        out.push_str("}\n");
    }

    fn gen_stmt(stmt: &Stmt, out: &mut String) {
        match stmt {
            Stmt::Let(name, _, type_, value, _) => {
                out.push_str("    ");
                if let Some(t) = type_ {
                    out.push_str(Self::type_to_c(t));
                } else {
                    out.push_str("auto");
                }
                out.push(' ');
                out.push_str(name);
                out.push_str(" = ");
                Self::gen_expr(value, out);
                out.push_str(";\n");
            }
            Stmt::Return(expr, _) => {
                out.push_str("    return");
                if let Some(e) = expr {
                    out.push(' ');
                    Self::gen_expr(e, out);
                }
                out.push_str(";\n");
            }
            Stmt::Expr(expr, _) => {
                out.push_str("    ");
                Self::gen_expr(expr, out);
                out.push_str(";\n");
            }
            Stmt::If(cond, then_block, else_block, _) => {
                out.push_str("    if (");
                Self::gen_expr(cond, out);
                out.push_str(") {\n");
                for stmt in &then_block.stmts {
                    Self::gen_stmt(stmt, out);
                }
                if let Some(else_b) = else_block {
                    out.push_str("    } else ");
                    if else_b.stmts.len() == 1
                        && matches!(else_b.stmts.first(), Some(Stmt::If(..)))
                    {
                        let stmt = &else_b.stmts[0];
                        Self::gen_stmt(stmt, out);
                    } else {
                        out.push_str("{\n");
                        for stmt in &else_b.stmts {
                            Self::gen_stmt(stmt, out);
                        }
                        out.push_str("    }\n");
                    }
                } else {
                    out.push_str("    }\n");
                }
            }
        }
    }

    fn gen_fn_def(&mut self, fn_def: &FnDef) {
        let ret_type = Self::type_to_c(&fn_def.return_type);

        if fn_def.name == "main" {
            self.write("int main(int argc, char** argv)");
        } else {
            let sig = format!("{} {}(", ret_type, fn_def.name);
            self.write(&sig);
            for (i, param) in fn_def.params.iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                let pt = Self::type_to_c(&param.type_);
                write!(self.output, "{} {}", pt, param.name).unwrap();
            }
            self.output.push(')');
        }

        self.output.push_str(" {\n");
        self.push();
        for stmt in &fn_def.body.stmts {
            let s = Self::stmt_to_string(stmt);
            self.write(&s);
        }
        self.pop();
        self.writeln("}");
    }

    fn stmt_to_string(stmt: &Stmt) -> String {
        let mut out = String::new();
        Self::gen_stmt(stmt, &mut out);
        out
    }

    pub fn generate(prog: &Program) -> String {
        let mut out = String::new();

        out.push_str("// Generated by Uno compiler\n");
        out.push_str("#include <stdint.h>\n");
        out.push_str("#include <stdbool.h>\n");
        out.push_str("#include <stdio.h>\n\n");

        for fn_def in &prog.functions {
            Self::gen_fn_decl(fn_def, &mut out);
        }
        out.push('\n');

        for fn_def in &prog.functions {
            let mut cg = Codegen::new();
            cg.gen_fn_def(fn_def);
            out.push_str(&cg.output);
            out.push('\n');
        }

        out
    }

    fn gen_fn_decl(fn_def: &FnDef, out: &mut String) {
        let ret_type = Self::type_to_c(&fn_def.return_type);
        if fn_def.name == "main" {
            out.push_str("int main(int argc, char** argv);\n");
        } else {
            out.push_str(ret_type);
            out.push(' ');
            out.push_str(&fn_def.name);
            out.push('(');
            for (i, param) in fn_def.params.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                let pt = Self::type_to_c(&param.type_);
                out.push_str(pt);
                out.push(' ');
                out.push_str(&param.name);
            }
            out.push_str(");\n");
        }
    }
}

impl Backend for Codegen {
    type Output = String;
    type Err = std::convert::Infallible;

    fn name(&self) -> &str {
        "C codegen"
    }

    fn generate(&mut self, prog: &Program) -> Result<Self::Output, Self::Err> {
        Ok(Self::generate(prog))
    }
}
