use std::{collections::BTreeMap, fmt, marker::PhantomData, slice};

use super::{Interpreter, eval_op};
use crate::expr::{Context, Error, Op, ops::BlockIndex, stack::Stack};

// --- DicePool + DicePoolEvaluator (preset dice rolls) ---

/// Immutable pool of preset dice values, keyed by die sides.
/// Create a [`DicePoolIter`] via [`iter()`](DicePool::iter) for evaluation.
#[derive(Debug, Clone, Default)]
pub struct DicePool(BTreeMap<u32, Vec<u32>>);

impl DicePool {
    /// Create an iterator that yields preset values in order.
    pub fn iter(&self) -> DicePoolIter<'_> {
        DicePoolIter(self.0.iter().map(|(&k, v)| (k, v.iter())).collect())
    }

    /// Returns `true` if this pool has no dice values.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for DicePool {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut first = true;
        for (sides, rolls) in &self.0 {
            if !first {
                f.write_str("; ")?;
            }
            first = false;
            write!(f, "d{sides}: ")?;
            for (i, roll) in rolls.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                write!(f, "{roll}")?;
            }
        }
        Ok(())
    }
}

impl From<BTreeMap<u32, Vec<u32>>> for DicePool {
    fn from(pool: BTreeMap<u32, Vec<u32>>) -> Self {
        Self(pool)
    }
}

/// Borrowing iterator over a [`DicePool`] that yields preset values via
/// `roll()`.
pub struct DicePoolIter<'a>(BTreeMap<u32, slice::Iter<'a, u32>>);

impl DicePoolIter<'_> {
    /// Draw the next preset value for a die with the given number of sides.
    pub fn roll(&mut self, sides: u32) -> Option<u32> {
        self.0.get_mut(&sides)?.next().copied()
    }
}

pub(crate) struct DicePoolEvaluator<'a, 'p, Var, Ctx> {
    stack: Stack<i32>,
    ctx: &'a mut Ctx,
    pool: &'a mut DicePoolIter<'p>,
    _var: PhantomData<Var>,
}

impl<'a, 'p, Var, Ctx> DicePoolEvaluator<'a, 'p, Var, Ctx> {
    pub fn new(ctx: &'a mut Ctx, pool: &'a mut DicePoolIter<'p>) -> Self {
        Self {
            stack: Stack::new(),
            ctx,
            pool,
            _var: PhantomData,
        }
    }
}

impl<Var: Copy + fmt::Display, Ctx: Context<Var, i32>> Interpreter<Var, i32>
    for DicePoolEvaluator<'_, '_, Var, Ctx>
{
    type Output = i32;

    fn exec(&mut self, op: Op<Var, i32>) -> Result<Option<BlockIndex>, Error> {
        match op {
            Op::PushVar(var) => {
                self.stack.push(self.ctx.resolve(var)?);
                Ok(None)
            }
            Op::Assign(var) => {
                self.ctx.assign(var, *self.stack.top()?)?;
                Ok(None)
            }
            Op::Roll => {
                let (count, sides) = self.stack.pop2()?;
                let sides_u32 = sides as u32;
                for _ in 0..count {
                    let value = self
                        .pool
                        .roll(sides_u32)
                        .ok_or(Error::DicePoolExhausted(sides_u32))?;
                    self.stack.push(value as i32);
                }
                self.stack.push(sides);
                self.stack.push(count);
                Ok(None)
            }
            op => eval_op(&mut self.stack, op),
        }
    }

    fn finish(self) -> Result<i32, Error> {
        self.stack.result()
    }
}
