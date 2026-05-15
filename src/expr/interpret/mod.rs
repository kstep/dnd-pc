mod analyze;
mod dice;
mod evaluator;
mod formatter;

pub use self::{
    analyze::ExprAnalysis,
    dice::{DicePool, DicePoolEvaluator},
    evaluator::{Evaluator, ReadOnlyEvaluator},
    formatter::Formatter,
};
use crate::expr::{
    Error, Op, VarGroup, avg_hp,
    group::{GroupCursor, NoGroup},
    ops::{BLOCK_ERROR, BLOCK_NOOP, BlockIndex},
    stack::Stack,
    traits::{Context, ResolveGroup},
};

pub type CursorStack<Var> = Stack<GroupCursor<Var>>;

pub trait Interpreter<Var, Val, Grp = NoGroup<Var>> {
    type Output;

    /// Execute a single op. Returns `None` to continue, or `Some(block_idx)`
    /// to tell `run_block` to evaluate that sub-block next.
    fn exec(&mut self, op: Op<Var, Val, Grp>) -> Result<Option<BlockIndex>, Error>;
    fn finish(self) -> Result<Self::Output, Error>;

    fn run(mut self, ops: impl Iterator<Item = Op<Var, Val, Grp>>) -> Result<Self::Output, Error>
    where
        Self: Sized,
    {
        for op in ops {
            self.exec(op)?;
        }
        self.finish()
    }
}

// --- Shared arithmetic/dice evaluation ---

/// Generate a random number in 1..=sides using getrandom.
fn roll_die(sides: i32) -> Result<i32, Error> {
    if sides <= 0 {
        return Err(Error::InvalidDieSides(sides));
    }
    let n = getrandom::u32().map_err(|_| Error::RngFailed)?;
    Ok((n % sides as u32 + 1) as i32)
}

pub fn eval_op<Var, Grp: VarGroup<Var = Var>>(
    stack: &mut Stack<i32>,
    op: Op<Var, i32, Grp>,
) -> Result<Option<BlockIndex>, Error> {
    match op {
        Op::PushConst(n) => stack.push(n),
        Op::BinOp(bin_op) => {
            let (a, b) = stack.pop2()?;
            stack.push(bin_op.eval(a, b)?);
        }
        Op::Roll => {
            let (count, sides) = stack.pop2()?;
            for _ in 0..count {
                stack.push(roll_die(sides)?);
            }
            stack.push(sides);
            stack.push(count);
        }
        Op::KeepMax(n) => {
            let count = stack.pop()? as usize;
            let _sides = stack.pop()?;
            stack.pop_n_reduce(count, |vals| {
                vals.sort_unstable_by(|a, b| b.cmp(a));
                vals[..n as usize].iter().sum()
            })?;
        }
        Op::KeepMin(n) => {
            let count = stack.pop()? as usize;
            let _sides = stack.pop()?;
            stack.pop_n_reduce(count, |vals| {
                vals.sort_unstable();
                vals[..n as usize].iter().sum()
            })?;
        }
        Op::DropMax(n) => {
            let count = stack.pop()? as usize;
            let _sides = stack.pop()?;
            stack.pop_n_reduce(count, |vals| {
                vals.sort_unstable_by(|a, b| b.cmp(a));
                vals[n as usize..].iter().sum()
            })?;
        }
        Op::DropMin(n) => {
            let count = stack.pop()? as usize;
            let _sides = stack.pop()?;
            stack.pop_n_reduce(count, |vals| {
                vals.sort_unstable();
                vals[n as usize..].iter().sum()
            })?;
        }
        Op::Sum => {
            let count = stack.pop()? as usize;
            let _sides = stack.pop()?;
            stack.pop_n_reduce(count, |vals| vals.iter().sum())?;
        }
        Op::Explode => {
            let count = stack.pop()? as usize;
            let sides = stack.pop()?;
            stack.pop_n_reduce(count, |vals| {
                let mut sum = 0;
                for &mut v in vals.iter_mut() {
                    sum += v;
                    if v < sides {
                        break;
                    }
                }
                sum
            })?;
        }
        Op::AvgHp => {
            let sides = stack.pop()?;
            stack.push(avg_hp(sides));
        }
        Op::Not => {
            let a = stack.pop()?;
            stack.push((a == 0) as i32);
        }
        Op::Cmp(cmp) => {
            let (a, b) = stack.pop2()?;
            stack.push(cmp.eval(a, b) as i32);
        }
        Op::In => {
            let (a, b, c) = stack.pop3()?;
            stack.push((b <= a && a <= c) as i32);
        }
        Op::Eval(idx) => return eval_block(idx),
        Op::EvalIf(then_idx, else_idx) => {
            let cond = stack.pop()?;
            return eval_block(if cond != 0 { then_idx } else { else_idx });
        }
        Op::Each(_) | Op::Next => {
            // Each/Next need ctx for ResolveGroup; handled in handle_loop_op.
            unreachable!("Each/Next must be intercepted by handle_loop_op before eval_op");
        }
        Op::Tier(count) => {
            let var = stack.pop()?;
            let mut result = 0;
            let mut found = false;
            for _ in 0..count {
                let value = stack.pop()?;
                let threshold = stack.pop()?;
                if !found && var >= threshold {
                    result = value;
                    found = true;
                }
            }
            stack.push(result);
        }
        Op::PushVar(_) | Op::AssignVar(_) | Op::PushGroup(_, _) | Op::AssignGroup(_, _) => {
            unreachable!()
        }
    }
    Ok(None)
}

/// Resolve a block index: [`BLOCK_NOOP`] = noop, [`BLOCK_ERROR`] = error,
/// otherwise run block.
pub fn eval_block(idx: BlockIndex) -> Result<Option<BlockIndex>, Error> {
    match idx {
        BLOCK_NOOP => Ok(None),
        BLOCK_ERROR => Err(Error::GuardFailed),
        _ => Ok(Some(idx)),
    }
}

/// Resolve a group op's column at the cursor's current row, or emit
/// `GroupOutOfBounds` if no cursor is live or the column is out of range.
pub fn cursor_var<Var: Copy>(iter_stack: &CursorStack<Var>, col: u8) -> Result<Var, Error> {
    let cursor = iter_stack.top().map_err(|_| Error::GroupOutOfBounds)?;
    cursor.col(col).copied().ok_or(Error::GroupOutOfBounds)
}

/// Handle the two push ops (`PushVar`, `PushGroup`) by resolving the var
/// through `ctx` and pushing onto `stack`. Returns `Ok(None)` if handled,
/// `Ok(Some(op))` for any other op.
pub fn handle_push_op<Var, Grp, Ctx>(
    op: Op<Var, i32, Grp>,
    stack: &mut Stack<i32>,
    iter_stack: &CursorStack<Var>,
    ctx: &Ctx,
) -> Result<Option<Op<Var, i32, Grp>>, Error>
where
    Var: Copy,
    Grp: VarGroup<Var = Var>,
    Ctx: Context<Var, i32>,
{
    match op {
        Op::PushVar(var) => {
            stack.push(ctx.resolve(var)?);
            Ok(None)
        }
        Op::PushGroup(_grp, col) => {
            let var = cursor_var(iter_stack, col)?;
            stack.push(ctx.resolve(var)?);
            Ok(None)
        }
        other => Ok(Some(other)),
    }
}

/// Handle the two assign ops (`AssignVar`, `AssignGroup`) by forwarding the
/// stack top to `ctx.assign`. Returns `Ok(None)` if handled, `Ok(Some(op))`
/// for any other op.
pub fn handle_assign_op<Var, Grp, Ctx>(
    op: Op<Var, i32, Grp>,
    stack: &Stack<i32>,
    iter_stack: &CursorStack<Var>,
    ctx: &mut Ctx,
) -> Result<Option<Op<Var, i32, Grp>>, Error>
where
    Var: Copy,
    Grp: VarGroup<Var = Var>,
    Ctx: Context<Var, i32>,
{
    match op {
        Op::AssignVar(var) => {
            ctx.assign(var, *stack.top()?)?;
            Ok(None)
        }
        Op::AssignGroup(_grp, col) => {
            let var = cursor_var(iter_stack, col)?;
            ctx.assign(var, *stack.top()?)?;
            Ok(None)
        }
        other => Ok(Some(other)),
    }
}

/// Handle loop ops (`Each`, `Next`). On `Each`, materialise the subgroup's
/// rows via `ctx.resolve_group` into a fresh cursor; push live-flag onto
/// stack. On `Next`, advance the top cursor; pop on exhaustion.
pub fn handle_loop_op<Var, Grp, Ctx>(
    op: Op<Var, i32, Grp>,
    stack: &mut Stack<i32>,
    iter_stack: &mut CursorStack<Var>,
    ctx: &Ctx,
) -> Result<Option<Op<Var, i32, Grp>>, Error>
where
    Var: Copy,
    Grp: VarGroup<Var = Var>,
    Ctx: ResolveGroup<Grp>,
{
    match op {
        Op::Each(subgrp) => {
            let cursor = GroupCursor::build(ctx, &subgrp);
            let live = cursor.is_live();
            if live {
                iter_stack.push(cursor);
            }
            stack.push(live as i32);
            Ok(None)
        }
        Op::Next => {
            let live = iter_stack
                .top_mut()
                .map(|cursor| cursor.advance())
                .unwrap_or(false);
            if !live {
                let _ = iter_stack.pop();
            }
            stack.push(live as i32);
            Ok(None)
        }
        other => Ok(Some(other)),
    }
}

/// Compose [`handle_push_op`], [`handle_assign_op`] and [`handle_loop_op`].
/// Returns `Ok(None)` if the op was one of the context-delegating ops;
/// `Ok(Some(op))` for anything else (eval_op handles the remainder).
pub fn handle_context_op<Var, Grp, Ctx>(
    op: Op<Var, i32, Grp>,
    stack: &mut Stack<i32>,
    iter_stack: &mut CursorStack<Var>,
    ctx: &mut Ctx,
) -> Result<Option<Op<Var, i32, Grp>>, Error>
where
    Var: Copy,
    Grp: VarGroup<Var = Var>,
    Ctx: Context<Var, i32> + ResolveGroup<Grp>,
{
    let Some(op) = handle_push_op(op, stack, iter_stack, &*ctx)? else {
        return Ok(None);
    };
    let Some(op) = handle_assign_op(op, stack, iter_stack, ctx)? else {
        return Ok(None);
    };
    handle_loop_op(op, stack, iter_stack, &*ctx)
}
