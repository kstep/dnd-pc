use std::{collections::BTreeMap, fmt, marker::PhantomData, slice};

use crate::expr::{Context, Error, Op, avg_hp, stack::Stack};

pub trait Interpreter<Var, Val> {
    type Output;

    /// Execute a single op. Returns `None` to continue, or `Some(block_idx)`
    /// to tell `run_block` to evaluate that sub-block next.
    fn exec(&mut self, op: Op<Var, Val>) -> Result<Option<usize>, Error>;
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

// --- Evaluator (apply mode, mutable context) ---

pub(super) struct Evaluator<'a, Var, Ctx> {
    stack: Stack<i32>,
    ctx: &'a mut Ctx,
    _var: PhantomData<Var>,
}

impl<'a, Var, Ctx> Evaluator<'a, Var, Ctx> {
    pub fn new(ctx: &'a mut Ctx) -> Self {
        Self {
            stack: Stack::new(),
            ctx,
            _var: PhantomData,
        }
    }
}

impl<Var: Copy + fmt::Display, Ctx: Context<Var, i32>> Interpreter<Var, i32>
    for Evaluator<'_, Var, Ctx>
{
    type Output = i32;

    fn exec(&mut self, op: Op<Var, i32>) -> Result<Option<usize>, Error> {
        match op {
            Op::PushVar(var) => {
                self.stack.push(self.ctx.resolve(var)?);
                Ok(None)
            }
            Op::Assign(var) => {
                self.ctx.assign(var, *self.stack.top()?)?;
                Ok(None)
            }
            op => eval_op(&mut self.stack, op),
        }
    }

    fn finish(self) -> Result<i32, Error> {
        self.stack.result()
    }
}

// --- ReadOnlyEvaluator (eval mode, immutable context) ---

pub(super) struct ReadOnlyEvaluator<'a, Var, Ctx> {
    stack: Stack<i32>,
    ctx: &'a Ctx,
    _var: PhantomData<Var>,
}

impl<'a, Var, Ctx> ReadOnlyEvaluator<'a, Var, Ctx> {
    pub fn new(ctx: &'a Ctx) -> Self {
        Self {
            stack: Stack::new(),
            ctx,
            _var: PhantomData,
        }
    }
}

impl<Var: Copy + fmt::Display, Ctx: Context<Var, i32>> Interpreter<Var, i32>
    for ReadOnlyEvaluator<'_, Var, Ctx>
{
    type Output = i32;

    fn exec(&mut self, op: Op<Var, i32>) -> Result<Option<usize>, Error> {
        match op {
            Op::PushVar(var) => {
                self.stack.push(self.ctx.resolve(var)?);
                Ok(None)
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

fn eval_op<Var>(stack: &mut Stack<i32>, op: Op<Var, i32>) -> Result<Option<usize>, Error> {
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

/// Resolve a block index: 0 = noop, 255 = error, otherwise run block.
fn eval_block(idx: u8) -> Result<Option<usize>, Error> {
    match idx {
        0 => Ok(None),
        255 => Err(Error::InvalidBlock(idx)),
        _ => Ok(Some(idx as usize)),
    }
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

pub(super) struct DicePoolEvaluator<'a, 'p, Var, Ctx> {
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

    fn exec(&mut self, op: Op<Var, i32>) -> Result<Option<usize>, Error> {
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

// --- DiceRollsCollector (collects dice requirements without rolling) ---

pub(super) struct DiceRollsCollector<'a, Var, Ctx> {
    stack: Stack<i32>,
    ctx: &'a Ctx,
    rolls: BTreeMap<u32, u32>,
    _var: PhantomData<Var>,
}

impl<'a, Var, Ctx> DiceRollsCollector<'a, Var, Ctx> {
    pub fn new(ctx: &'a Ctx) -> Self {
        Self {
            stack: Stack::new(),
            ctx,
            rolls: BTreeMap::new(),
            _var: PhantomData,
        }
    }
}

impl<Var: Copy + fmt::Display, Ctx: Context<Var, i32>> Interpreter<Var, i32>
    for DiceRollsCollector<'_, Var, Ctx>
{
    type Output = BTreeMap<u32, u32>;

    fn exec(&mut self, op: Op<Var, i32>) -> Result<Option<usize>, Error> {
        match op {
            Op::PushVar(var) => {
                self.stack.push(self.ctx.resolve(var)?);
                Ok(None)
            }
            Op::Assign(_) => Ok(None),
            Op::Roll => {
                let (count, sides) = self.stack.pop2()?;
                if count > 0 && sides > 0 {
                    *self.rolls.entry(sides as u32).or_insert(0) += count as u32;
                }
                for _ in 0..count {
                    self.stack.push(avg_hp(sides));
                }
                self.stack.push(count);
                Ok(None)
            }
            op => eval_op(&mut self.stack, op),
        }
    }

    fn finish(self) -> Result<BTreeMap<u32, u32>, Error> {
        Ok(self.rolls)
    }
}

// --- Formatter (Display interpreter) ---

struct Frag {
    text: String,
    prec: u8, // 0=assign, 1=or, 2=and, 3=cmp, 4=add/sub, 5=mul/div, 6=unary, 7=atom
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
        let right_min = if right_strict { prec + 1 } else { prec };
        let left = Self::wrap(a, prec);
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

    pub fn pop_text(&mut self) -> Result<String, Error> {
        Ok(self.stack.pop()?.text)
    }

    pub fn push_atom(&mut self, text: String) {
        self.push(text, 7);
    }
}

impl<Var: Copy + fmt::Display, Val: Copy + fmt::Display> Interpreter<Var, Val> for Formatter {
    type Output = String;

    fn exec(&mut self, op: Op<Var, Val>) -> Result<Option<usize>, Error> {
        match op {
            Op::PushConst(n) => {
                self.push(n.to_string(), 7);
            }
            Op::PushVar(var) => {
                self.push(var.to_string(), 7);
            }
            Op::Add => self.binary_op("+", 4, false)?,
            Op::Sub => self.binary_op("-", 4, true)?,
            Op::Mul => self.binary_op("*", 5, false)?,
            Op::DivFloor => self.binary_op("/", 5, true)?,
            Op::DivCeil => self.binary_op("\\", 5, true)?,
            Op::Mod => self.binary_op("%", 5, true)?,
            Op::Min => self.binary_func("min")?,
            Op::Max => self.binary_func("max")?,
            Op::Roll => {
                let sides = self.stack.pop()?;
                let count = self.stack.pop()?;
                let sides = Self::wrap(sides, 7);
                let text = if count.text == "1" {
                    format!("d{sides}")
                } else {
                    let count = Self::wrap(count, 7);
                    format!("{count}d{sides}")
                };
                self.push(text, 7);
            }
            Op::Sum => {
                // Follows Roll; roll fragment is already on stack
            }
            Op::KeepMax(n) => {
                let roll = self.stack.pop()?;
                self.push(format!("{}kh{n}", roll.text), 7);
            }
            Op::KeepMin(n) => {
                let roll = self.stack.pop()?;
                self.push(format!("{}kl{n}", roll.text), 7);
            }
            Op::DropMax(n) => {
                let roll = self.stack.pop()?;
                self.push(format!("{}dh{n}", roll.text), 7);
            }
            Op::DropMin(n) => {
                let roll = self.stack.pop()?;
                self.push(format!("{}dl{n}", roll.text), 7);
            }
            Op::AvgHp => {
                let sides = self.stack.pop()?;
                self.push(format!("avg_hp({})", sides.text), 7);
            }
            Op::And => self.binary_op("and", 2, false)?,
            Op::Or => self.binary_op("or", 1, false)?,
            Op::Not => {
                let a = self.stack.pop()?;
                let text = Self::wrap(a, 6);
                self.push(format!("not {text}"), 6);
            }
            Op::Cmp(cmp) => self.binary_op(cmp.symbol(), 3, false)?,
            Op::Assign(var) => {
                let val = self.stack.pop()?;
                self.push(format!("{var} = {}", val.text), 0);
            }
            Op::In => {
                let c = self.stack.pop()?;
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                self.push(format!("in({}, {}, {})", a.text, b.text, c.text), 3);
            }
            Op::Eval(_) | Op::EvalIf(_, _) => {} // intercepted by format_block
        }
        Ok(None)
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
