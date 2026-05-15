use crate::{
    expr::{self, Context, ResolveGroup},
    model::{Attribute, AttributeGroup},
};

/// Mutable `Context` wrapper that intercepts `@ARG(n)` lookups and returns
/// `args[n]`. All other vars fall through to the wrapped context. Used by
/// feature apply to run assign ops into a real `Character`.
pub struct WithArgs<'a, C> {
    pub inner: &'a mut C,
    pub args: &'a [i32],
}

impl<C: ResolveGroup<AttributeGroup>> ResolveGroup<AttributeGroup> for WithArgs<'_, C> {
    fn resolve_group<'b>(
        &'b self,
        grp: &AttributeGroup,
    ) -> Box<dyn Iterator<Item = Vec<Attribute>> + 'b> {
        <C as ResolveGroup<AttributeGroup>>::resolve_group(self.inner, grp)
    }
}

impl<C: Context<Attribute, i32>> Context<Attribute, i32> for WithArgs<'_, C> {
    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        match var {
            Attribute::Arg(n) => self
                .args
                .get(n as usize)
                .copied()
                .ok_or_else(|| expr::Error::unsupported_var(var)),
            other => self.inner.resolve(other),
        }
    }

    fn assign(&mut self, var: Attribute, value: i32) -> Result<(), expr::Error> {
        self.inner.assign(var, value)
    }
}

/// Read-only variant of [`WithArgs`] — takes `&'a C` to avoid cloning the
/// underlying context for guard checks. `assign` returns `Err` because
/// `eval_lenient` never invokes it in read-only mode; any caller that does
/// is misusing the wrapper.
pub struct WithArgsRef<'a, C> {
    pub inner: &'a C,
    pub args: &'a [i32],
}

impl<C: ResolveGroup<AttributeGroup>> ResolveGroup<AttributeGroup> for WithArgsRef<'_, C> {
    fn resolve_group<'b>(
        &'b self,
        grp: &AttributeGroup,
    ) -> Box<dyn Iterator<Item = Vec<Attribute>> + 'b> {
        <C as ResolveGroup<AttributeGroup>>::resolve_group(self.inner, grp)
    }
}

impl<C: Context<Attribute, i32>> Context<Attribute, i32> for WithArgsRef<'_, C> {
    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        match var {
            Attribute::Arg(n) => self
                .args
                .get(n as usize)
                .copied()
                .ok_or_else(|| expr::Error::unsupported_var(var)),
            other => self.inner.resolve(other),
        }
    }

    fn assign(&mut self, var: Attribute, _value: i32) -> Result<(), expr::Error> {
        Err(expr::Error::assign_at_eval(var))
    }
}
