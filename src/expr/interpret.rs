use std::{collections::BTreeMap, fmt, marker::PhantomData, slice};

use crate::expr::{Context, Error, Op, avg_hp, stack::Stack};

pub trait Interpreter<Var, Val> {
    type Output;
    fn exec(&mut self, op: Op<Var, Val>) -> Result<(), Error>;
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

    fn exec(&mut self, op: Op<Var, i32>) -> Result<(), Error> {
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

    fn exec(&mut self, op: Op<Var, i32>) -> Result<(), Error> {
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

fn eval_op<Var>(stack: &mut Stack<i32>, op: Op<Var, i32>) -> Result<(), Error> {
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
        Op::Lt => {
            let (a, b) = stack.pop2()?;
            stack.push((a < b) as i32);
        }
        Op::Gt => {
            let (a, b) = stack.pop2()?;
            stack.push((a > b) as i32);
        }
        Op::Le => {
            let (a, b) = stack.pop2()?;
            stack.push((a <= b) as i32);
        }
        Op::Ge => {
            let (a, b) = stack.pop2()?;
            stack.push((a >= b) as i32);
        }
        Op::CmpEq => {
            let (a, b) = stack.pop2()?;
            stack.push((a == b) as i32);
        }
        Op::CmpNe => {
            let (a, b) = stack.pop2()?;
            stack.push((a != b) as i32);
        }
        Op::If => {
            let else_val = stack.pop()?;
            let then_val = stack.pop()?;
            let cond = stack.pop()?;
            stack.push(if cond != 0 { then_val } else { else_val });
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

    fn exec(&mut self, op: Op<Var, i32>) -> Result<(), Error> {
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
    prec: u8, // 0=assign, 1=or, 2=and, 3=cmp, 4=add/sub, 5=mul/div, 6=unary, 7=atom
    /// If this frag is a binary op where the left operand was an atom,
    /// stores (left_text, op_symbol, rhs_text) for compound assignment
    /// detection.
    compound: Option<(String, String, String)>,
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
        self.stack.push(Frag {
            text,
            prec,
            compound: None,
        });
    }

    fn wrap_ref(frag: &Frag, min_prec: u8) -> String {
        if frag.prec < min_prec {
            format!("({})", frag.text)
        } else {
            frag.text.clone()
        }
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
        let compound = if a.prec == 7 && a.compound.is_none() {
            // Direct: left operand is a plain atom (variable or constant).
            // Store b.text without wrapping — compound assignment implicitly
            // groups the entire RHS, so outer parens are unnecessary.
            Some((a.text.clone(), sym.to_string(), b.text.clone()))
        } else if let Some((ref left_var, ref first_op, ref prev_rhs)) = a.compound {
            // Propagate through addition chains: x + (a ± b) = (x + a) ± b
            if *first_op == "+" && prec == 4 {
                let right = Self::wrap_ref(&b, right_min);
                Some((
                    left_var.clone(),
                    first_op.clone(),
                    format!("{prev_rhs} {sym} {right}"),
                ))
            } else {
                None
            }
        } else {
            None
        };
        let left = Self::wrap(a, prec);
        let right = Self::wrap(b, right_min);
        self.stack.push(Frag {
            text: format!("{left} {sym} {right}"),
            prec,
            compound,
        });
        Ok(())
    }

    fn binary_func(&mut self, name: &str) -> Result<(), Error> {
        let b = self.stack.pop()?;
        let a = self.stack.pop()?;
        self.push(format!("{name}({}, {})", a.text, b.text), 3);
        Ok(())
    }
}

impl<Var: fmt::Display, Val: fmt::Display> Interpreter<Var, Val> for Formatter {
    type Output = String;

    fn exec(&mut self, op: Op<Var, Val>) -> Result<(), Error> {
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
                let text = if count.text == "1" {
                    format!("d{}", sides.text)
                } else {
                    format!("{}d{}", count.text, sides.text)
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
            Op::Lt => self.binary_op("<", 3, false)?,
            Op::Gt => self.binary_op(">", 3, false)?,
            Op::Le => self.binary_op("<=", 3, false)?,
            Op::Ge => self.binary_op(">=", 3, false)?,
            Op::CmpEq => self.binary_op("==", 3, false)?,
            Op::CmpNe => self.binary_op("!=", 3, false)?,
            Op::If => {
                let else_val = self.stack.pop()?;
                let then_val = self.stack.pop()?;
                let cond = self.stack.pop()?;
                self.push(
                    format!("if({}, {}, {})", cond.text, then_val.text, else_val.text),
                    7,
                );
            }
            Op::Assign(var) => {
                let val = self.stack.pop()?;
                let var_str = var.to_string();
                let text = if let Some((ref left_var, ref op, ref rhs)) = val.compound {
                    if *left_var == var_str {
                        format!("{var} {op}= {rhs}")
                    } else {
                        format!("{var} = {}", val.text)
                    }
                } else {
                    format!("{var} = {}", val.text)
                };
                self.push(text, 0);
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
