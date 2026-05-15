use std::{collections::BTreeMap, fmt, marker::PhantomData, slice};

use serde::{Deserialize, Serialize};

use crate::expr::{
    Context, Error, Op, ResolveGroup, VarGroup,
    interpret::{CursorStack, Interpreter, eval_op, handle_context_op},
    ops::BlockIndex,
    stack::Stack,
};

// --- DicePool + DicePoolEvaluator (preset dice rolls) ---

/// Immutable pool of preset dice values, keyed by die sides.
/// Create a [`DicePoolIter`] via [`iter()`](DicePool::iter) for evaluation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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

    /// Look up preset values for a given die side count.
    pub fn get(&self, sides: u32) -> &[u32] {
        self.0.get(&sides).map_or(&[], Vec::as_slice)
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

pub struct DicePoolEvaluator<'a, 'p, Var, Ctx> {
    stack: Stack<i32>,
    iter_stack: CursorStack<Var>,
    ctx: &'a mut Ctx,
    pool: &'a mut DicePoolIter<'p>,
    _var: PhantomData<Var>,
}

impl<'a, 'p, Var, Ctx> DicePoolEvaluator<'a, 'p, Var, Ctx> {
    pub fn new(ctx: &'a mut Ctx, pool: &'a mut DicePoolIter<'p>) -> Self {
        Self {
            stack: Stack::new(),
            iter_stack: CursorStack::new(),
            ctx,
            pool,
            _var: PhantomData,
        }
    }
}

impl<Var, Ctx, Grp> Interpreter<Var, i32, Grp> for DicePoolEvaluator<'_, '_, Var, Ctx>
where
    Var: Copy + fmt::Display,
    Grp: VarGroup<Var = Var>,
    Ctx: Context<Var, i32> + ResolveGroup<Grp>,
{
    type Output = i32;

    fn exec(&mut self, op: Op<Var, i32, Grp>) -> Result<Option<BlockIndex>, Error> {
        if let Op::Roll = op {
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
            return Ok(None);
        }
        match handle_context_op(op, &mut self.stack, &mut self.iter_stack, self.ctx)? {
            None => Ok(None),
            Some(op) => eval_op(&mut self.stack, op),
        }
    }

    fn finish(self) -> Result<i32, Error> {
        self.stack.result()
    }
}
