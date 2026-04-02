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
    Error, Op, avg_hp,
    ops::{BLOCK_ERROR, BLOCK_NOOP, BlockIndex},
    stack::Stack,
};

pub trait Interpreter<Var, Val> {
    type Output;

    /// Execute a single op. Returns `None` to continue, or `Some(block_idx)`
    /// to tell `run_block` to evaluate that sub-block next.
    fn exec(&mut self, op: Op<Var, Val>) -> Result<Option<BlockIndex>, Error>;
    fn finish(self) -> Result<Self::Output, Error>;

    fn run(mut self, ops: impl Iterator<Item = Op<Var, Val>>) -> Result<Self::Output, Error>
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

fn eval_op<Var>(stack: &mut Stack<i32>, op: Op<Var, i32>) -> Result<Option<BlockIndex>, Error> {
    match op {
        Op::PushConst(n) => stack.push(n),
        Op::Add => {
            let (a, b) = stack.pop2()?;
            stack.push(a + b);
        }
        Op::Sub => {
            let (a, b) = stack.pop2()?;
            stack.push(a - b);
        }
        Op::Mul => {
            let (a, b) = stack.pop2()?;
            stack.push(a * b);
        }
        Op::DivFloor => {
            let (a, b) = stack.pop2()?;
            if b == 0 {
                return Err(Error::DivisionByZero);
            }
            stack.push(a.div_euclid(b));
        }
        Op::DivCeil => {
            let (a, b) = stack.pop2()?;
            if b == 0 {
                return Err(Error::DivisionByZero);
            }
            let d = a.div_euclid(b);
            let r = a.rem_euclid(b);
            stack.push(if r != 0 { d + 1 } else { d });
        }
        Op::Mod => {
            let (a, b) = stack.pop2()?;
            if b == 0 {
                return Err(Error::DivisionByZero);
            }
            stack.push(a.rem_euclid(b));
        }
        Op::Min => {
            let (a, b) = stack.pop2()?;
            stack.push(a.min(b));
        }
        Op::Max => {
            let (a, b) = stack.pop2()?;
            stack.push(a.max(b));
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
        Op::And => {
            let (a, b) = stack.pop2()?;
            stack.push((a != 0 && b != 0) as i32);
        }
        Op::Or => {
            let (a, b) = stack.pop2()?;
            stack.push((a != 0 || b != 0) as i32);
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
        Op::PushVar(_) | Op::Assign(_) => unreachable!(),
    }
    Ok(None)
}

/// Resolve a block index: [`BLOCK_NOOP`] = noop, [`BLOCK_ERROR`] = error,
/// otherwise run block.
fn eval_block(idx: BlockIndex) -> Result<Option<BlockIndex>, Error> {
    match idx {
        BLOCK_NOOP => Ok(None),
        BLOCK_ERROR => Err(Error::GuardFailed),
        _ => Ok(Some(idx)),
    }
}
