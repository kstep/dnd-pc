use std::{collections::BTreeMap, fmt, marker::PhantomData, slice};

use crate::expr::{Context, Error, Op, stack::Stack};

pub trait Interpreter<Var> {
    type Output;
    fn exec(&mut self, op: Op<Var>) -> Result<(), Error>;
    fn finish(self) -> Result<Self::Output, Error>;

    fn run(mut self, ops: impl Iterator<Item = Op<Var>>) -> Result<Self::Output, Error>
    where
        Self: Sized,
    {
        for op in ops {
            self.exec(op)?;
        }
        self.finish()
    }
}

// --- Evaluator (apply mode, mutable context) ---

pub(super) struct Evaluator<'a, Var, Ctx: Context<Var>> {
    stack: Stack<i32>,
    ctx: &'a mut Ctx,
    _var: PhantomData<Var>,
}

impl<'a, Var, Ctx: Context<Var>> Evaluator<'a, Var, Ctx> {
    pub fn new(ctx: &'a mut Ctx) -> Self {
        Self {
            stack: Stack::new(),
            ctx,
            _var: PhantomData,
        }
    }
}

impl<Var: Copy + fmt::Display, Ctx: Context<Var>> Interpreter<Var> for Evaluator<'_, Var, Ctx> {
    type Output = i32;

    fn exec(&mut self, op: Op<Var>) -> Result<(), Error> {
        match op {
            Op::PushVar(var) => {
                self.stack.push(self.ctx.resolve(var)?);
                Ok(())
            }
            Op::Assign(var) => self.ctx.assign(var, *self.stack.top()?),
            op => eval_op(&mut self.stack, op),
        }
    }

    fn finish(self) -> Result<i32, Error> {
        self.stack.result()
    }
}

// --- ReadOnlyEvaluator (eval mode, immutable context) ---

pub(super) struct ReadOnlyEvaluator<'a, Var, Ctx: Context<Var>> {
    stack: Stack<i32>,
    ctx: &'a Ctx,
    _var: PhantomData<Var>,
}

impl<'a, Var, Ctx: Context<Var>> ReadOnlyEvaluator<'a, Var, Ctx> {
    pub fn new(ctx: &'a Ctx) -> Self {
        Self {
            stack: Stack::new(),
            ctx,
            _var: PhantomData,
        }
    }
}

impl<Var: Copy + fmt::Display, Ctx: Context<Var>> Interpreter<Var>
    for ReadOnlyEvaluator<'_, Var, Ctx>
{
    type Output = i32;

    fn exec(&mut self, op: Op<Var>) -> Result<(), Error> {
        match op {
            Op::PushVar(var) => {
                self.stack.push(self.ctx.resolve(var)?);
                Ok(())
            }
            Op::Assign(var) => Err(Error::assign_at_eval(var)),
            op => eval_op(&mut self.stack, op),
        }
    }

    fn finish(self) -> Result<i32, Error> {
        self.stack.result()
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

fn eval_op<Var>(stack: &mut Stack<i32>, op: Op<Var>) -> Result<(), Error> {
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
            stack.push(count);
        }
        Op::KeepMax(n) => {
            let count = stack.pop()? as usize;
            stack.pop_n_reduce(count, |vals| {
                vals.sort_unstable_by(|a, b| b.cmp(a));
                vals[..n as usize].iter().sum()
            })?;
        }
        Op::KeepMin(n) => {
            let count = stack.pop()? as usize;
            stack.pop_n_reduce(count, |vals| {
                vals.sort_unstable();
                vals[..n as usize].iter().sum()
            })?;
        }
        Op::DropMax(n) => {
            let count = stack.pop()? as usize;
            stack.pop_n_reduce(count, |vals| {
                vals.sort_unstable_by(|a, b| b.cmp(a));
                vals[n as usize..].iter().sum()
            })?;
        }
        Op::DropMin(n) => {
            let count = stack.pop()? as usize;
            stack.pop_n_reduce(count, |vals| {
                vals.sort_unstable();
                vals[n as usize..].iter().sum()
            })?;
        }
        Op::Sum => {
            let count = stack.pop()? as usize;
            stack.pop_n_reduce(count, |vals| vals.iter().sum())?;
        }
        Op::PushVar(_) | Op::Assign(_) => unreachable!(),
    }
    Ok(())
}

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

pub(super) struct DicePoolEvaluator<'a, 'p, Var, Ctx: Context<Var>> {
    stack: Stack<i32>,
    ctx: &'a mut Ctx,
    pool: &'a mut DicePoolIter<'p>,
    _var: PhantomData<Var>,
}

impl<'a, 'p, Var, Ctx: Context<Var>> DicePoolEvaluator<'a, 'p, Var, Ctx> {
    pub fn new(ctx: &'a mut Ctx, pool: &'a mut DicePoolIter<'p>) -> Self {
        Self {
            stack: Stack::new(),
            ctx,
            pool,
            _var: PhantomData,
        }
    }
}

impl<Var: Copy + fmt::Display, Ctx: Context<Var>> Interpreter<Var>
    for DicePoolEvaluator<'_, '_, Var, Ctx>
{
    type Output = i32;

    fn exec(&mut self, op: Op<Var>) -> Result<(), Error> {
        match op {
            Op::PushVar(var) => {
                self.stack.push(self.ctx.resolve(var)?);
                Ok(())
            }
            Op::Assign(var) => self.ctx.assign(var, *self.stack.top()?),
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
                self.stack.push(count);
                Ok(())
            }
            op => eval_op(&mut self.stack, op),
        }
    }

    fn finish(self) -> Result<i32, Error> {
        self.stack.result()
    }
}

// --- Formatter (Display interpreter) ---

struct Frag {
    text: String,
    prec: u8, // 0=assign, 1=add/sub, 2=mul/div, 3=atom
}

pub(super) struct Formatter {
    stack: Stack<Frag>,
}

impl Formatter {
    pub fn new() -> Self {
        Self {
            stack: Stack::new(),
        }
    }

    fn push(&mut self, text: String, prec: u8) {
        self.stack.push(Frag { text, prec });
    }

    fn wrap(frag: Frag, min_prec: u8) -> String {
        if frag.prec < min_prec {
            format!("({})", frag.text)
        } else {
            frag.text
        }
    }

    fn binary_op(&mut self, sym: &str, prec: u8, right_strict: bool) -> Result<(), Error> {
        let b = self.stack.pop()?;
        let a = self.stack.pop()?;
        let left = Self::wrap(a, prec);
        let right_min = if right_strict { prec + 1 } else { prec };
        let right = Self::wrap(b, right_min);
        self.push(format!("{left} {sym} {right}"), prec);
        Ok(())
    }

    fn binary_func(&mut self, name: &str) -> Result<(), Error> {
        let b = self.stack.pop()?;
        let a = self.stack.pop()?;
        self.push(format!("{name}({}, {})", a.text, b.text), 3);
        Ok(())
    }
}

impl<Var: Copy + fmt::Display> Interpreter<Var> for Formatter {
    type Output = String;

    fn exec(&mut self, op: Op<Var>) -> Result<(), Error> {
        match op {
            Op::PushConst(n) => {
                let text = if n < 0 {
                    format!("({n})")
                } else {
                    n.to_string()
                };
                self.push(text, 3);
            }
            Op::PushVar(var) => {
                self.push(var.to_string(), 3);
            }
            Op::Add => self.binary_op("+", 1, false)?,
            Op::Sub => self.binary_op("-", 1, true)?,
            Op::Mul => self.binary_op("*", 2, false)?,
            Op::DivFloor => self.binary_op("/", 2, true)?,
            Op::DivCeil => self.binary_op("\\", 2, true)?,
            Op::Mod => self.binary_op("%", 2, true)?,
            Op::Min => self.binary_func("min")?,
            Op::Max => self.binary_func("max")?,
            Op::Roll => {
                let sides = self.stack.pop()?;
                let count = self.stack.pop()?;
                let text = if count.text == "1" {
                    format!("d{}", sides.text)
                } else {
                    format!("{}d{}", count.text, sides.text)
                };
                self.push(text, 3);
            }
            Op::Sum => {
                // Follows Roll; roll fragment is already on stack
            }
            Op::KeepMax(n) => {
                let roll = self.stack.pop()?;
                self.push(format!("{}kh{n}", roll.text), 3);
            }
            Op::KeepMin(n) => {
                let roll = self.stack.pop()?;
                self.push(format!("{}kl{n}", roll.text), 3);
            }
            Op::DropMax(n) => {
                let roll = self.stack.pop()?;
                self.push(format!("{}dh{n}", roll.text), 3);
            }
            Op::DropMin(n) => {
                let roll = self.stack.pop()?;
                self.push(format!("{}dl{n}", roll.text), 3);
            }
            Op::Assign(var) => {
                let val = self.stack.pop()?;
                self.push(format!("{var} = {}", val.text), 0);
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<String, Error> {
        Ok(self
            .stack
            .iter()
            .flat_map(|frag| [frag.text.as_str(), "; "])
            .take(self.stack.len() * 2 - 1)
            .collect::<String>())
    }
}
