pub mod lower;

pub type LocalId = String;

#[derive(Debug, Clone, PartialEq)]
pub enum IrType {
    Bool,
    Uint(usize),
}

#[derive(Debug, Clone)]
pub enum IrConstant {
    Int(u128),
    Bool(bool),
}

#[derive(Debug, Clone)]
pub enum Value {
    Local(LocalId),
    Const(IrConstant),
}

#[derive(Debug, Clone)]
pub enum Inst {
    Add(LocalId, IrType, Value, Value),
    Sub(LocalId, IrType, Value, Value),
    Mul(LocalId, IrType, Value, Value),
    Div(LocalId, IrType, Value, Value),
    Mod(LocalId, IrType, Value, Value),
    Eq(LocalId, Value, Value),
    Neq(LocalId, Value, Value),
    Lt(LocalId, Value, Value),
    Gt(LocalId, Value, Value),
    Le(LocalId, Value, Value),
    Ge(LocalId, Value, Value),
    And(LocalId, Value, Value),
    Or(LocalId, Value, Value),
    Not(LocalId, Value),
    Neg(LocalId, IrType, Value),
    LoadConst(LocalId, IrConstant, IrType),
    Load(LocalId, IrType, LocalId),
    Store(LocalId, IrType, Value),
    Call(LocalId, IrType, String, Vec<Value>),
    If(Value, Vec<Inst>, Vec<Inst>),
    While(Vec<Inst>),
    Loop(Vec<Inst>),
    Break,
    Return(Option<Value>),
    Drop(Value),
}

#[derive(Debug, Clone)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<(LocalId, IrType)>,
    pub return_type: IrType,
    pub locals: Vec<(LocalId, IrType)>,
    pub insts: Vec<Inst>,
}

#[derive(Debug, Clone)]
pub struct IrProgram {
    pub functions: Vec<IrFunction>,
}

pub trait IrBackend {
    type Output;
    type Error: std::error::Error;
    fn name(&self) -> &str;
    fn generate(&mut self, ir: &IrProgram) -> Result<Self::Output, Self::Error>;
}

pub fn pretty_ir(ir: &IrProgram) -> String {
    let mut out = String::new();
    for func in &ir.functions {
        out.push_str(&format!("fn {}(", func.name));
        for (i, (name, ty)) in func.params.iter().enumerate() {
            if i > 0 { out.push_str(", "); }
            out.push_str(&format!("{name}: {ty:?}"));
        }
        out.push_str(&format!(") -> {:?}", func.return_type));
        out.push_str(" {\n");
        for (name, ty) in &func.locals {
            out.push_str(&format!("  let {}: {:?};\n", name, ty));
        }
        for inst in &func.insts {
            out.push_str("  ");
            out.push_str(&pretty_inst(inst));
            out.push('\n');
        }
        out.push_str("}\n\n");
    }
    out
}

fn pretty_inst(inst: &Inst) -> String {
    match inst {
        Inst::Add(d, _, a, b) => format!("{} = add({}, {})", d, v(a), v(b)),
        Inst::Sub(d, _, a, b) => format!("{} = sub({}, {})", d, v(a), v(b)),
        Inst::Mul(d, _, a, b) => format!("{} = mul({}, {})", d, v(a), v(b)),
        Inst::Div(d, _, a, b) => format!("{} = div({}, {})", d, v(a), v(b)),
        Inst::Mod(d, _, a, b) => format!("{} = mod({}, {})", d, v(a), v(b)),
        Inst::Eq(d, a, b) => format!("{} = eq({}, {})", d, v(a), v(b)),
        Inst::Neq(d, a, b) => format!("{} = neq({}, {})", d, v(a), v(b)),
        Inst::Lt(d, a, b) => format!("{} = lt({}, {})", d, v(a), v(b)),
        Inst::Gt(d, a, b) => format!("{} = gt({}, {})", d, v(a), v(b)),
        Inst::Le(d, a, b) => format!("{} = le({}, {})", d, v(a), v(b)),
        Inst::Ge(d, a, b) => format!("{} = ge({}, {})", d, v(a), v(b)),
        Inst::And(d, a, b) => format!("{} = and({}, {})", d, v(a), v(b)),
        Inst::Or(d, a, b) => format!("{} = or({}, {})", d, v(a), v(b)),
        Inst::Not(d, a) => format!("{} = not({})", d, v(a)),
        Inst::Neg(d, _, a) => format!("{} = neg({})", d, v(a)),
        Inst::LoadConst(d, c, _) => format!("{} = const({})", d, pc(c)),
        Inst::Load(d, _, src) => format!("{} = load({})", d, src),
        Inst::Store(dst, _, val) => format!("store({}, {})", dst, v(val)),
        Inst::Call(d, _, name, args) => {
            let a: Vec<_> = args.iter().map(v).collect();
            format!("{} = call {}({})", d, name, a.join(", "))
        }
        Inst::If(cond, then_, else_) => {
            let t: Vec<_> = then_.iter().map(pretty_inst).collect();
            let e: Vec<_> = else_.iter().map(pretty_inst).collect();
            format!(
                "if {} {{\n    {}\n  }} else {{\n    {}\n  }}",
                v(cond),
                t.join("\n    "),
                e.join("\n    ")
            )
        }
        Inst::While(body) => {
            let b: Vec<_> = body.iter().map(pretty_inst).collect();
            format!("while {{\n    {}\n  }}", b.join("\n    "))
        }
        Inst::Loop(body) => {
            let b: Vec<_> = body.iter().map(pretty_inst).collect();
            format!("loop {{\n    {}\n  }}", b.join("\n    "))
        }
        Inst::Break => "break".to_string(),
        Inst::Return(rv) => match rv {
            Some(val) => format!("return {}", v(val)),
            None => "return".to_string(),
        },
        Inst::Drop(dv) => format!("drop({})", v(dv)),
    }
}

fn v(val: &Value) -> String {
    match val {
        Value::Local(id) => id.clone(),
        Value::Const(c) => pc(c),
    }
}

fn pc(c: &IrConstant) -> String {
    match c {
        IrConstant::Int(n) => n.to_string(),
        IrConstant::Bool(b) => b.to_string(),
    }
}
