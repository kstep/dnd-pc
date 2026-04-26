use crate::{
    expr,
    model::{Attribute, Character},
};

/// Read-only context that resolves ARG variables from a slice for validation.
pub struct ArgsContext<'a> {
    pub character: &'a Character,
    pub args: &'a [i32],
}

impl expr::Context<Attribute, i32> for ArgsContext<'_> {
    fn assign(&mut self, _var: Attribute, _value: i32) -> Result<(), expr::Error> {
        Ok(())
    }

    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        match var {
            Attribute::Arg(n) => self
                .args
                .get(n as usize)
                .copied()
                .ok_or_else(|| expr::Error::unsupported_var(var)),
            other => self.character.resolve(other),
        }
    }
}

/// Mutating context that simulates applying an assignment to a character
/// while also capturing every assign call (var, value) for display.
pub struct PreviewContext<'a> {
    pub character: &'a mut Character,
    pub captured: Vec<(Attribute, i32)>,
}

impl expr::Context<Attribute, i32> for PreviewContext<'_> {
    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        self.character.resolve(var)
    }

    fn assign(&mut self, var: Attribute, value: i32) -> Result<(), expr::Error> {
        if self
            .character
            .resolve(var)
            .ok()
            .is_none_or(|prev| prev != value)
        {
            self.captured.push((var, value));
        }
        // Scoped attrs (CasterAbility, SlotPool, ...) live per-feature and
        // are ReadOnly on a global Character — capture for display, skip
        // the proxy. Other attrs propagate normally so the assign chain and
        // cumulative pipeline still surface real write errors.
        if var.is_scoped() {
            Ok(())
        } else {
            self.character.assign(var, value)
        }
    }
}
