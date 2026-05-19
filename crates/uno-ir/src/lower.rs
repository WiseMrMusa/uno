use crate::{Inst, IrConstant, IrFunction, IrProgram, IrType, LocalId, Value};
use uno_syntax::ast::*;

pub struct LowerCtx {
    temp_counter: usize,
}

impl LowerCtx {
    pub fn new() -> Self {
        LowerCtx { temp_counter: 0 }
    }

    fn fresh(&mut self) -> LocalId {
        let id = self.temp_counter;
        self.temp_counter += 1;
        format!("_t{id}")
    }
}

pub fn lower(program: &Program) -> Result<IrProgram, String> {
    let mut ctx = LowerCtx::new();
    let mut functions = Vec::new();
    for fn_def in &program.functions {
        functions.push(lower_fn(&mut ctx, fn_def)?);
    }
    Ok(IrProgram { functions })
}

fn lower_fn(ctx: &mut LowerCtx, fn_def: &FnDef) -> Result<IrFunction, String> {
    let return_type = ast_type_to_ir(&fn_def.return_type);
    let params: Vec<(LocalId, IrType)> = fn_def
        .params
        .iter()
        .map(|p| (p.name.clone(), ast_type_to_ir(&p.type_)))
        .collect();

    let mut lctx = LocalCtx::new();
    for (name, ty) in &params {
        lctx.declare(name, ty.clone());
    }

    let mut insts = Vec::new();
    for stmt in &fn_def.body.stmts {
        lower_stmt(&mut lctx, &mut insts, ctx, stmt)?;
    }

    Ok(IrFunction {
        name: fn_def.name.clone(),
        params,
        return_type,
        locals: lctx.locals,
        insts,
    })
}

struct LocalCtx {
    locals: Vec<(LocalId, IrType)>,
}

impl LocalCtx {
    fn new() -> Self {
        LocalCtx { locals: Vec::new() }
    }

    fn declare(&mut self, name: &str, ty: IrType) -> LocalId {
        if !self.locals.iter().any(|(n, _)| n == name) {
            self.locals.push((name.to_string(), ty.clone()));
        }
        name.to_string()
    }

    fn get_type(&self, name: &str) -> IrType {
        self.locals
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t.clone())
            .unwrap_or(IrType::Uint(32))
    }
}

fn lower_stmt(
    lctx: &mut LocalCtx,
    insts: &mut Vec<Inst>,
    ctx: &mut LowerCtx,
    stmt: &Stmt,
) -> Result<(), String> {
    match stmt {
        Stmt::Let(name, _is_mut, type_, value, _span) => {
            let ty = match type_ {
                Some(t) => ast_type_to_ir(t),
                None => infer_type(value),
            };
            lctx.declare(name, ty.clone());
            let (mut val_insts, val) = lower_expr(lctx, ctx, value, &ty)?;
            insts.append(&mut val_insts);
            insts.push(Inst::Store(name.clone(), ty, val));
            Ok(())
        }
        Stmt::Assign(name, value, _span) => {
            let ty = lctx.get_type(name);
            let (mut val_insts, val) = lower_expr(lctx, ctx, value, &ty)?;
            insts.append(&mut val_insts);
            insts.push(Inst::Store(name.clone(), ty, val));
            Ok(())
        }
        Stmt::Return(expr, _span) => match expr {
            Some(e) => {
                let ty = infer_type(e);
                let (mut val_insts, val) = lower_expr(lctx, ctx, e, &ty)?;
                insts.append(&mut val_insts);
                insts.push(Inst::Return(Some(val)));
                Ok(())
            }
            None => {
                insts.push(Inst::Return(None));
                Ok(())
            }
        },
        Stmt::Expr(expr, _span) => {
            let ty = infer_type(expr);
            let (mut val_insts, val) = lower_expr(lctx, ctx, expr, &ty)?;
            insts.append(&mut val_insts);
            insts.push(Inst::Drop(val));
            Ok(())
        }
        Stmt::If(cond, then_block, else_block, _span) => {
            let cond_ty = IrType::Bool;
            let (mut cond_insts, cond_val) = lower_expr(lctx, ctx, cond, &cond_ty)?;

            let mut then_insts = Vec::new();
            for s in &then_block.stmts {
                lower_stmt(lctx, &mut then_insts, ctx, s)?;
            }

            let mut else_insts = Vec::new();
            if let Some(b) = else_block {
                for s in &b.stmts {
                    lower_stmt(lctx, &mut else_insts, ctx, s)?;
                }
            }

            insts.append(&mut cond_insts);
            insts.push(Inst::If(cond_val, then_insts, else_insts));
            Ok(())
        }
        Stmt::While(cond, body, _span) => {
            let cond_ty = IrType::Bool;
            let (mut cond_insts, cond_val) = lower_expr(lctx, ctx, cond, &cond_ty)?;

            let mut body_insts = Vec::new();
            for s in &body.stmts {
                lower_stmt(lctx, &mut body_insts, ctx, s)?;
            }

            let mut while_body = Vec::new();
            while_body.append(&mut cond_insts);
            while_body.push(Inst::If(cond_val, body_insts, vec![Inst::Break]));
            insts.push(Inst::While(while_body));
            Ok(())
        }
        Stmt::Loop(body, _span) => {
            let mut body_insts = Vec::new();
            for s in &body.stmts {
                lower_stmt(lctx, &mut body_insts, ctx, s)?;
            }

            insts.push(Inst::Loop(body_insts));
            Ok(())
        }
        Stmt::Break(_span) => {
            insts.push(Inst::Break);
            Ok(())
        }
    }
}

fn lower_expr(
    lctx: &mut LocalCtx,
    ctx: &mut LowerCtx,
    expr: &Expr,
    expected: &IrType,
) -> Result<(Vec<Inst>, Value), String> {
    match expr {
        Expr::Literal(_val, _span) => {
            let ty = infer_type(expr);
            let (c, _) = parse_ir_literal(_val);
            let local = lctx.declare(&ctx.fresh(), ty.clone());
            Ok((vec![Inst::LoadConst(local.clone(), c, ty)], Value::Local(local)))
        }
        Expr::Ident(name, _span) => match name.as_str() {
            "true" => {
                let local = lctx.declare(&ctx.fresh(), IrType::Bool);
                Ok((
                    vec![Inst::LoadConst(local.clone(), IrConstant::Bool(true), IrType::Bool)],
                    Value::Local(local),
                ))
            }
            "false" => {
                let local = lctx.declare(&ctx.fresh(), IrType::Bool);
                Ok((
                    vec![Inst::LoadConst(local.clone(), IrConstant::Bool(false), IrType::Bool)],
                    Value::Local(local),
                ))
            }
            _ => {
                let ty = lctx.get_type(name);
                let local = lctx.declare(&ctx.fresh(), ty.clone());
                Ok((vec![Inst::Load(local.clone(), ty, name.clone())], Value::Local(local)))
            }
        },
        Expr::BinaryOp(left, op, right, _span) => {
            let ty = match op {
                BinOp::Eq | BinOp::Neq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
                | BinOp::And | BinOp::Or => IrType::Bool,
                _ => expected.clone(),
            };
            let (mut l_insts, l_val) = lower_expr(lctx, ctx, left, &ty)?;
            let (mut r_insts, r_val) = lower_expr(lctx, ctx, right, &ty)?;
            let dest = lctx.declare(&ctx.fresh(), ty.clone());
            let op_inst = op_to_ir(&dest, op, l_val, r_val, &ty);
            let mut all = Vec::new();
            all.append(&mut l_insts);
            all.append(&mut r_insts);
            all.push(op_inst);
            Ok((all, Value::Local(dest)))
        }
        Expr::UnaryOp(op, operand, _span) => {
            let ty = match op {
                UnOp::Not => IrType::Bool,
                UnOp::Neg => expected.clone(),
            };
            let (mut op_insts, op_val) = lower_expr(lctx, ctx, operand, &ty)?;
            let dest = lctx.declare(&ctx.fresh(), ty.clone());
            let inst = match op {
                UnOp::Neg => Inst::Neg(dest.clone(), ty, op_val),
                UnOp::Not => Inst::Not(dest.clone(), op_val),
            };
            op_insts.push(inst);
            Ok((op_insts, Value::Local(dest)))
        }
        Expr::Block(block, _span) => {
            let mut block_insts = Vec::new();
            let count = block.stmts.len();
            for (i, stmt) in block.stmts.iter().enumerate() {
                if i == count.saturating_sub(1) {
                    match stmt {
                        Stmt::Expr(e, _) => {
                            return lower_expr(lctx, ctx, e, expected);
                        }
                        _ => {
                            lower_stmt(lctx, &mut block_insts, ctx, stmt)?;
                            let v = lctx.declare(&ctx.fresh(), expected.clone());
                            block_insts.push(Inst::LoadConst(
                                v.clone(),
                                IrConstant::Int(0),
                                expected.clone(),
                            ));
                            return Ok((block_insts, Value::Local(v)));
                        }
                    }
                } else {
                    lower_stmt(lctx, &mut block_insts, ctx, stmt)?;
                }
            }
            let v = lctx.declare(&ctx.fresh(), expected.clone());
            block_insts.push(Inst::LoadConst(v.clone(), IrConstant::Int(0), expected.clone()));
            Ok((block_insts, Value::Local(v)))
        }
        Expr::FnCall(name, args, _span) => {
            let ret_ty = expected.clone();
            let mut all_insts = Vec::new();
            let mut arg_vals = Vec::new();
            for arg in args {
                let arg_ty = infer_type(arg);
                let (mut ai, av) = lower_expr(lctx, ctx, arg, &arg_ty)?;
                all_insts.append(&mut ai);
                arg_vals.push(av);
            }
            let dest = lctx.declare(&ctx.fresh(), ret_ty.clone());
            all_insts.push(Inst::Call(dest.clone(), ret_ty, name.clone(), arg_vals));
            Ok((all_insts, Value::Local(dest)))
        }
        Expr::Paren(inner, _span) => lower_expr(lctx, ctx, inner, expected),
    }
}

fn op_to_ir(dest: &LocalId, op: &BinOp, l: Value, r: Value, ty: &IrType) -> Inst {
    match op {
        BinOp::Add => Inst::Add(dest.clone(), ty.clone(), l, r),
        BinOp::Sub => Inst::Sub(dest.clone(), ty.clone(), l, r),
        BinOp::Mul => Inst::Mul(dest.clone(), ty.clone(), l, r),
        BinOp::Div => Inst::Div(dest.clone(), ty.clone(), l, r),
        BinOp::Mod => Inst::Mod(dest.clone(), ty.clone(), l, r),
        BinOp::Eq => Inst::Eq(dest.clone(), l, r),
        BinOp::Neq => Inst::Neq(dest.clone(), l, r),
        BinOp::Lt => Inst::Lt(dest.clone(), l, r),
        BinOp::Gt => Inst::Gt(dest.clone(), l, r),
        BinOp::Le => Inst::Le(dest.clone(), l, r),
        BinOp::Ge => Inst::Ge(dest.clone(), l, r),
        BinOp::And => Inst::And(dest.clone(), l, r),
        BinOp::Or => Inst::Or(dest.clone(), l, r),
    }
}

pub fn infer_type(expr: &Expr) -> IrType {
    match expr {
        Expr::Literal(val, _) => {
            let has_64 = val.contains("u64") || val.contains("u128");
            if has_64 { IrType::Uint(64) } else { IrType::Uint(32) }
        }
        Expr::Ident(name, _) => match name.as_str() {
            "true" | "false" => IrType::Bool,
            _ => IrType::Uint(32),
        },
        Expr::BinaryOp(_, op, _, _) => match op {
            BinOp::Eq | BinOp::Neq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
            | BinOp::And | BinOp::Or => IrType::Bool,
            _ => IrType::Uint(32),
        },
        Expr::UnaryOp(UnOp::Not, _, _) => IrType::Bool,
        Expr::UnaryOp(UnOp::Neg, _, _) => IrType::Uint(32),
        Expr::Block(block, _) => {
            for stmt in block.stmts.iter().rev() {
                if let Stmt::Expr(e, _) = stmt {
                    return infer_type(e);
                }
            }
            IrType::Uint(32)
        }
        Expr::FnCall(_, _, _) => IrType::Uint(32),
        Expr::Paren(inner, _) => infer_type(inner),
    }
}

fn parse_ir_literal(val: &str) -> (IrConstant, bool) {
    let hex = val.starts_with("0x") || val.starts_with("0X");
    let num = if hex {
        let digits: String = val[2..].chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        if digits.is_empty() { 0 } else { u128::from_str_radix(&digits, 16).unwrap_or(0) }
    } else {
        let digits: String = val.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() { 0 } else { digits.parse().unwrap_or(0) }
    };
    (IrConstant::Int(num), false)
}

fn ast_type_to_ir(t: &Type) -> IrType {
    match t {
        Type::Bool => IrType::Bool,
        Type::Uint(n) => IrType::Uint(*n),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Program {
        let mut lexer = uno_lexer::Lexer::new(source.to_string());
        let tokens = lexer.tokenize();
        let mut parser = uno_parser::Parser::new(tokens);
        parser.parse_program().unwrap()
    }

    #[test]
    fn lower_empty_fn() {
        let prog = parse("fn main() -> u32 { return 0; }");
        let ir = lower(&prog).unwrap();
        assert_eq!(ir.functions.len(), 1);
        assert_eq!(ir.functions[0].name, "main");
    }

    #[test]
    fn lower_let() {
        let prog = parse("fn main() -> u32 { let x: u32 = 42; return x; }");
        let ir = lower(&prog).unwrap();
        let f = &ir.functions[0];
        assert!(f.locals.iter().any(|(n, _)| n == "x"));
        assert!(f.insts.iter().any(|i| matches!(i, Inst::Store(..))));
    }

    #[test]
    fn lower_binary_op() {
        let prog = parse("fn main() -> u32 { return 1 + 2; }");
        let ir = lower(&prog).unwrap();
        let f = &ir.functions[0];
        assert!(f.insts.iter().any(|i| matches!(i, Inst::Add(..))));
    }

    #[test]
    fn lower_if() {
        let prog = parse("fn main() -> u32 { if true { return 1; } else { return 2; } }");
        let ir = lower(&prog).unwrap();
        let f = &ir.functions[0];
        assert!(f.insts.iter().any(|i| matches!(i, Inst::If(..))));
    }

    #[test]
    fn lower_fn_call() {
        let prog = parse("fn foo() -> u32 { return 1; } fn main() -> u32 { return foo(); }");
        let ir = lower(&prog).unwrap();
        let f = &ir.functions[1];
        assert!(f.insts.iter().any(|i| matches!(i, Inst::Call(..))));
    }

    #[test]
    fn temps_are_declared() {
        let prog = parse("fn main() -> u32 { return 1 + 2; }");
        let ir = lower(&prog).unwrap();
        let f = &ir.functions[0];
        // binary op creates two temp locals for 1 and 2, plus one for result
        assert!(f.locals.iter().any(|(n, _)| n == "_t0"));
        assert!(f.locals.iter().any(|(n, _)| n == "_t1"));
        assert!(f.locals.iter().any(|(n, _)| n == "_t2"));
    }

    #[test]
    fn lower_while() {
        let prog = parse("fn main() -> u32 { while true { return 0; } return 1; }");
        let ir = lower(&prog).unwrap();
        let f = &ir.functions[0];
        assert!(f.insts.iter().any(|i| matches!(i, Inst::While(..))));
    }

    #[test]
    fn lower_loop() {
        let prog = parse("fn main() -> u32 { loop { break; } return 0; }");
        let ir = lower(&prog).unwrap();
        let f = &ir.functions[0];
        assert!(f.insts.iter().any(|i| matches!(i, Inst::Loop(..))));
        let has_break = f.insts.iter().any(|i| {
            if let Inst::Loop(body) = i {
                body.iter().any(|b| matches!(b, Inst::Break))
            } else {
                false
            }
        });
        assert!(has_break);
    }

    #[test]
    fn lower_assign() {
        let prog = parse("fn main() -> u32 { let mut x: u32 = 0; x = 5; return x; }");
        let ir = lower(&prog).unwrap();
        let f = &ir.functions[0];
        assert!(f.locals.iter().any(|(n, _)| n == "x"));
        assert!(f.insts.iter().any(|i| matches!(i, Inst::Store(..))));
    }
}
