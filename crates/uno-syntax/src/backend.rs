use crate::ast::Program;

pub trait Backend {
    type Output;
    type Err: std::error::Error;

    fn name(&self) -> &str;
    fn generate(&mut self, prog: &Program) -> Result<Self::Output, Self::Err>;
}
