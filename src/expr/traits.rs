use crate::expr::error::Error;

pub trait Context<Var, Val> {
    fn assign(&mut self, var: Var, value: Val) -> Result<(), Error>;
    fn resolve(&self, var: Var) -> Result<Val, Error>;
}

pub trait Eval<Var, Val> {
    type Output;

    fn eval(&self, ctx: &impl Context<Var, Val>) -> Self::Output;
    fn is_dynamic(&self) -> bool;
}
