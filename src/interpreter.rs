use std::collections::HashMap;
use uno_ir::{Inst, IrConstant, IrFunction, IrProgram, IrType, Value};

pub struct Interpreter {
    locals: HashMap<String, IrValue>,
    return_value: Option<IrValue>,
    breaking: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IrValue {
    Int(u128),
    Bool(bool),
}

impl IrValue {
    fn as_int(&self) -> u128 {
        match self {
            IrValue::Int(n) => *n,
            IrValue::Bool(b) => if *b { 1 } else { 0 },
        }
    }

    fn as_bool(&self) -> bool {
        match self {
            IrValue::Bool(b) => *b,
            IrValue::Int(0) => false,
            _ => true,
        }
    }
}

impl std::fmt::Display for IrValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IrValue::Int(n) => write!(f, "{n}"),
            IrValue::Bool(b) => write!(f, "{b}"),
        }
    }
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            locals: HashMap::new(),
            return_value: None,
            breaking: false,
        }
    }

    pub fn run(program: &IrProgram, verbose: bool) -> Result<i32, String> {
        let main = program.functions.iter().find(|f| f.name == "main")
            .ok_or("no main function found")?;

        if verbose { eprintln!("interpreting main..."); }

        let mut interp = Interpreter::new();
        let result = interp.call(program, main, &[])?;

        if verbose { eprintln!("main returned: {result}"); }

        match result {
            IrValue::Int(n) => Ok(n as i32),
            IrValue::Bool(b) => Ok(if b { 0 } else { 1 }),
        }
    }

    fn call(
        &mut self,
        program: &IrProgram,
        func: &IrFunction,
        args: &[IrValue],
    ) -> Result<IrValue, String> {
        let saved = self.locals.clone();
        let saved_ret = self.return_value.take();
        let saved_break = self.breaking;

        self.locals.clear();
        self.return_value = None;
        self.breaking = false;

        for ((name, _), arg_val) in func.params.iter().zip(args.iter()) {
            self.locals.insert(name.clone(), arg_val.clone());
        }

        for (name, _) in &func.locals {
            if !func.params.iter().any(|(pn, _)| pn == name) {
                self.locals.insert(name.clone(), IrValue::Int(0));
            }
        }

        match self.exec_insts(program, &func.insts) {
            Ok(()) => {
                let val = self.return_value.take().unwrap_or(IrValue::Int(0));
                self.locals = saved;
                self.return_value = saved_ret;
                self.breaking = saved_break;
                Ok(val)
            }
            Err(e) => {
                self.locals = saved;
                self.return_value = saved_ret;
                self.breaking = saved_break;
                Err(e)
            }
        }
    }

    fn exec_insts(&mut self, program: &IrProgram, insts: &[Inst]) -> Result<(), String> {
        for inst in insts {
            if self.return_value.is_some() || self.breaking {
                break;
            }
            self.exec_inst(program, inst)?;
        }
        Ok(())
    }

    fn exec_inst(&mut self, program: &IrProgram, inst: &Inst) -> Result<(), String> {
        match inst {
            Inst::Add(d, ty, a, b) => {
                let av = self.eval(a)?.as_int();
                let bv = self.eval(b)?.as_int();
                let result = Self::wrap_int(av.wrapping_add(bv), ty);
                self.locals.insert(d.clone(), result);
            }
            Inst::Sub(d, ty, a, b) => {
                let av = self.eval(a)?.as_int();
                let bv = self.eval(b)?.as_int();
                let result = Self::wrap_int(av.wrapping_sub(bv), ty);
                self.locals.insert(d.clone(), result);
            }
            Inst::Mul(d, ty, a, b) => {
                let av = self.eval(a)?.as_int();
                let bv = self.eval(b)?.as_int();
                let result = Self::wrap_int(av.wrapping_mul(bv), ty);
                self.locals.insert(d.clone(), result);
            }
            Inst::Div(d, ty, a, b) => {
                let av = self.eval(a)?.as_int();
                let bv = self.eval(b)?.as_int();
                if bv == 0 { return Err("division by zero".into()); }
                let result = Self::wrap_int(av / bv, ty);
                self.locals.insert(d.clone(), result);
            }
            Inst::Mod(d, ty, a, b) => {
                let av = self.eval(a)?.as_int();
                let bv = self.eval(b)?.as_int();
                if bv == 0 { return Err("modulo by zero".into()); }
                let result = Self::wrap_int(av % bv, ty);
                self.locals.insert(d.clone(), result);
            }
            Inst::Eq(d, a, b) => {
                let eq = self.eval(a)? == self.eval(b)?;
                self.locals.insert(d.clone(), IrValue::Bool(eq));
            }
            Inst::Neq(d, a, b) => {
                let eq = self.eval(a)? == self.eval(b)?;
                self.locals.insert(d.clone(), IrValue::Bool(!eq));
            }
            Inst::Lt(d, a, b) => {
                let result = IrValue::Bool(self.eval(a)?.as_int() < self.eval(b)?.as_int());
                self.locals.insert(d.clone(), result);
            }
            Inst::Gt(d, a, b) => {
                let result = IrValue::Bool(self.eval(a)?.as_int() > self.eval(b)?.as_int());
                self.locals.insert(d.clone(), result);
            }
            Inst::Le(d, a, b) => {
                let result = IrValue::Bool(self.eval(a)?.as_int() <= self.eval(b)?.as_int());
                self.locals.insert(d.clone(), result);
            }
            Inst::Ge(d, a, b) => {
                let result = IrValue::Bool(self.eval(a)?.as_int() >= self.eval(b)?.as_int());
                self.locals.insert(d.clone(), result);
            }
            Inst::And(d, a, b) => {
                let result = IrValue::Bool(self.eval(a)?.as_bool() && self.eval(b)?.as_bool());
                self.locals.insert(d.clone(), result);
            }
            Inst::Or(d, a, b) => {
                let result = IrValue::Bool(self.eval(a)?.as_bool() || self.eval(b)?.as_bool());
                self.locals.insert(d.clone(), result);
            }
            Inst::Not(d, a) => {
                let result = IrValue::Bool(!self.eval(a)?.as_bool());
                self.locals.insert(d.clone(), result);
            }
            Inst::Neg(d, ty, a) => {
                let av = self.eval(a)?.as_int();
                let result = Self::wrap_int(0u128.wrapping_sub(av), ty);
                self.locals.insert(d.clone(), result);
            }
            Inst::LoadConst(d, c, _ty) => {
                let val = const_to_value(c);
                self.locals.insert(d.clone(), val);
            }
            Inst::Load(d, _ty, src) => {
                let val = self.locals.get(src)
                    .cloned()
                    .unwrap_or(IrValue::Int(0));
                self.locals.insert(d.clone(), val);
            }
            Inst::Store(dst, _ty, val) => {
                let v = self.eval(val)?;
                self.locals.insert(dst.clone(), v);
            }
            Inst::Call(d, _ty, name, args) => {
                let callee = program.functions.iter().find(|f| &f.name == name)
                    .ok_or_else(|| format!("undefined function: {name}"))?;
                let arg_vals: Vec<IrValue> = args.iter()
                    .map(|a| self.eval(a))
                    .collect::<Result<Vec<_>, _>>()?;
                let result = self.call(program, callee, &arg_vals)?;
                self.locals.insert(d.clone(), result);
            }
            Inst::If(cond, then_, else_) => {
                if self.eval(cond)?.as_bool() {
                    self.exec_insts(program, then_)?;
                } else {
                    self.exec_insts(program, else_)?;
                }
            }
            Inst::While(body) => {
                loop {
                    if self.return_value.is_some() { break; }
                    self.exec_insts(program, body)?;
                    if self.breaking {
                        self.breaking = false;
                        break;
                    }
                }
            }
            Inst::Loop(body) => {
                loop {
                    if self.return_value.is_some() { break; }
                    self.exec_insts(program, body)?;
                    if self.breaking {
                        self.breaking = false;
                        break;
                    }
                }
            }
            Inst::Break => {
                self.breaking = true;
            }
            Inst::Return(v) => {
                let val = match v {
                    Some(val) => self.eval(val)?,
                    None => IrValue::Int(0),
                };
                self.return_value = Some(val);
            }
            Inst::Drop(_) => {
                // nothing to do in interpreter
            }
        }
        Ok(())
    }

    fn eval(&self, v: &Value) -> Result<IrValue, String> {
        match v {
            Value::Local(id) => {
                self.locals.get(id)
                    .cloned()
                    .ok_or_else(|| format!("undefined local: {id}"))
            }
            Value::Const(c) => Ok(const_to_value(c)),
        }
    }

    fn wrap_int(val: u128, ty: &IrType) -> IrValue {
        let mask = match ty {
            IrType::Uint(8) => 0xFF,
            IrType::Uint(16) => 0xFFFF,
            IrType::Uint(32) => 0xFFFF_FFFF,
            IrType::Uint(64) => !0,
            IrType::Uint(128) => !0,
            _ => !0,
        };
        IrValue::Int(val & mask)
    }
}

fn const_to_value(c: &IrConstant) -> IrValue {
    match c {
        IrConstant::Int(n) => IrValue::Int(*n),
        IrConstant::Bool(b) => IrValue::Bool(*b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interpret(source: &str) -> Result<i32, String> {
        let mut lexer = uno_lexer::Lexer::new(source.to_string());
        let tokens = lexer.tokenize();
        let mut parser = uno_parser::Parser::new(tokens);
        let program = parser.parse_program().map_err(|e| e.message)?;
        let ir = uno_ir::lower::lower(&program).map_err(|e| e.to_string())?;
        Interpreter::run(&ir, false)
    }

    #[test]
    fn simple_return() {
        assert_eq!(interpret("fn main() -> u32 { return 42; }").unwrap(), 42);
    }

    #[test]
    fn arithmetic() {
        assert_eq!(interpret("fn main() -> u32 { return 2 + 3 * 4; }").unwrap(), 14);
    }

    #[test]
    fn let_binding() {
        assert_eq!(interpret("fn main() -> u32 { let x: u32 = 10; return x + 5; }").unwrap(), 15);
    }

    #[test]
    fn if_else() {
        assert_eq!(interpret("fn main() -> u32 { if true { return 1; } else { return 2; } }").unwrap(), 1);
        assert_eq!(interpret("fn main() -> u32 { if false { return 1; } else { return 2; } }").unwrap(), 2);
    }

    #[test]
    fn while_loop() {
        let src = "fn main() -> u32 { let mut i: u32 = 0; while i < 5 { i = i + 1; } return i; }";
        assert_eq!(interpret(src).unwrap(), 5);
    }

    #[test]
    fn loop_break() {
        let src = "fn main() -> u32 { let mut i: u32 = 0; loop { i = i + 1; if i >= 10 { break; } } return i; }";
        assert_eq!(interpret(src).unwrap(), 10);
    }

    #[test]
    fn fn_call() {
        let src = "fn add(a: u32, b: u32) -> u32 { return a + b; } fn main() -> u32 { return add(3, 4); }";
        assert_eq!(interpret(src).unwrap(), 7);
    }

    #[test]
    fn fib() {
        let src = "fn fib(n: u32) -> u32 { if n <= 1 { return n; } return fib(n - 1) + fib(n - 2); } fn main() -> u32 { return fib(10); }";
        assert_eq!(interpret(src).unwrap(), 55);
    }

    #[test]
    fn boolean_ops() {
        assert_eq!(interpret("fn main() -> u32 { if true && !false { return 1; } return 0; }").unwrap(), 1);
        assert_eq!(interpret("fn main() -> u32 { if false || true { return 2; } return 0; }").unwrap(), 2);
    }

    #[test]
    fn comparison() {
        assert_eq!(interpret("fn main() -> u32 { if 5 > 3 { return 1; } return 0; }").unwrap(), 1);
        assert_eq!(interpret("fn main() -> u32 { if 3 >= 5 { return 1; } return 0; }").unwrap(), 0);
    }
}
