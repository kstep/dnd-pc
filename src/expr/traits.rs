use crate::expr::{error::Error, group::VarGroup};

pub trait Context<Var, Val> {
    fn assign(&mut self, var: Var, value: Val) -> Result<(), Error>;
    fn resolve(&self, var: Var) -> Result<Val, Error>;
}

/// Materialise rows of a `VarGroup` over `self`'s runtime state.
///
/// Concrete contexts (notably `Character`) implement this once per
/// supported group. Each row is a `Vec<G::Var>` of length
/// `1 + grp.columns().len()`, slot-aligned with `grp.schema()`:
/// `row[0] = Arg(iter_no)`, `row[k] = columns()[k-1](row_index)`.
pub trait ResolveGroup<G: VarGroup> {
    fn resolve_group<'a>(&'a self, grp: &G) -> Box<dyn Iterator<Item = Vec<G::Var>> + 'a>;
}

pub trait Eval<Var, Val, Grp: VarGroup<Var = Var>> {
    type Output;

    fn eval<C>(&self, ctx: &C) -> Self::Output
    where
        C: Context<Var, Val> + ResolveGroup<Grp>;
    fn is_dynamic(&self) -> bool;
}
