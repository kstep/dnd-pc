use serde::{Deserialize, Serialize};

use crate::expr::{
    Error,
    group::{NoGroup, VarSubgroup},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cmp {
    Lt, // <
    Le, // <=
    Gt, // >
    Ge, // >=
    Eq, // ==
    Ne, // !=
}

impl Cmp {
    pub fn eval(self, a: i32, b: i32) -> bool {
        match self {
            Self::Lt => a < b,
            Self::Le => a <= b,
            Self::Gt => a > b,
            Self::Ge => a >= b,
            Self::Eq => a == b,
            Self::Ne => a != b,
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::Eq => "==",
            Self::Ne => "!=",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOp {
    Add,      // +
    Sub,      // -
    Mul,      // *
    DivFloor, // /
    DivCeil,  // \
    Mod,      // %
    Min,      // min()
    Max,      // max()
    And,      // and
    Or,       // or
}

impl BinOp {
    pub fn eval(self, a: i32, b: i32) -> Result<i32, Error> {
        match self {
            Self::Add => Ok(a + b),
            Self::Sub => Ok(a - b),
            Self::Mul => Ok(a * b),
            Self::DivFloor => {
                if b == 0 {
                    return Err(Error::DivisionByZero);
                }
                Ok(a.div_euclid(b))
            }
            Self::DivCeil => {
                if b == 0 {
                    return Err(Error::DivisionByZero);
                }
                let d = a.div_euclid(b);
                let r = a.rem_euclid(b);
                Ok(if r != 0 { d + 1 } else { d })
            }
            Self::Mod => {
                if b == 0 {
                    return Err(Error::DivisionByZero);
                }
                Ok(a.rem_euclid(b))
            }
            Self::Min => Ok(a.min(b)),
            Self::Max => Ok(a.max(b)),
            Self::And => Ok((a != 0 && b != 0) as i32),
            Self::Or => Ok((a != 0 || b != 0) as i32),
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::DivFloor => "/",
            Self::DivCeil => "\\",
            Self::Mod => "%",
            Self::Min => "min",
            Self::Max => "max",
            Self::And => "and",
            Self::Or => "or",
        }
    }

    pub fn precedence(self) -> u8 {
        match self {
            Self::Or => 1,
            Self::And => 2,
            Self::Min | Self::Max => 3,
            Self::Add | Self::Sub => 4,
            Self::Mul | Self::DivFloor | Self::DivCeil | Self::Mod => 5,
        }
    }

    pub fn is_right_strict(self) -> bool {
        matches!(self, Self::Sub | Self::DivFloor | Self::DivCeil | Self::Mod)
    }

    pub fn is_func(self) -> bool {
        matches!(self, Self::Min | Self::Max)
    }

    pub fn compound_sym(self) -> Option<&'static str> {
        match self {
            Self::Add => Some("+"),
            Self::Sub => Some("-"),
            Self::Mul => Some("*"),
            Self::DivFloor => Some("/"),
            Self::DivCeil => Some("\\"),
            Self::Mod => Some("%"),
            _ => None,
        }
    }
}

/// Result of compound-assignment detection on an ops slice.
/// Contains the operator symbol and the index range of the RHS operand ops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompoundAssign {
    /// The compound operator symbol ("+", "-", "*", "/", "\\", "%").
    pub sym: &'static str,
    /// End of any stack-balanced prefix ops before the compound `Push(X)`
    /// — e.g. an inner `each(...)` side-effect loop that runs before the
    /// `X += rhs` statement. `0` when the statement is a plain compound
    /// with no prefix.
    pub prefix_end: usize,
    /// Start index of the RHS ops (after `Push(X)`).
    pub rhs_start: usize,
    /// End index of the RHS ops (before the combining op, exclusive).
    pub rhs_end: usize,
}

/// Type alias for block indices in expressions.
pub type BlockIndex = u8;

/// Block index of the main (entry) block.
pub const BLOCK_MAIN: BlockIndex = 0;

/// Block index meaning "no block" / no-op. Used as the else-branch of
/// `EvalIf` when there is no else clause.
pub const BLOCK_NOOP: BlockIndex = 0;

/// Block index that always triggers an error. Used by `guard()` as the
/// else-branch of `EvalIf` to signal a failed guard condition.
pub const BLOCK_ERROR: BlockIndex = BlockIndex::MAX;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op<Var, Val, Grp = NoGroup<Var>> {
    PushVar(Var),
    PushConst(Val),
    BinOp(BinOp),
    AvgHp,
    Roll,         // 2d20 -> 2 20 Roll Sum
    KeepMax(Val), // 2d20kh1 -> 2 20 Roll KeepMax(1)
    KeepMin(Val),
    DropMax(Val),
    DropMin(Val),
    Sum,
    Explode, // Nd8! → roll sequentially, sum until a die rolls less than max (sides)
    AssignVar(Var),
    Not, // logical not (0/1)
    Cmp(Cmp),
    In,                             // in(a, b, c) → b <= a && a <= c
    EvalIf(BlockIndex, BlockIndex), // if: pop cond, branch to then/else block
    Eval(BlockIndex),               // evaluate sub-block
    Each(VarSubgroup<Grp>),         // init loop: push cursor onto iter_stack, push 1/0
    Next,                           /* advance current cursor; push 1 if more rows, else
                                     * pop+push 0 */
    PushGroup(Grp, u8), // push iter_stack.top().col(col) via ctx.resolve
    AssignGroup(Grp, u8), /* pop value, assign to iter_stack.top().col(col) via
                         * ctx.assign */
    Tier(u8), // pop var, pop N*(threshold,value) pairs, push matching value
}

impl<Var: PartialEq, Val, Grp> Op<Var, Val, Grp> {
    /// Net stack-depth change of this op (+1 for push, -1 for binary, etc).
    fn stack_delta(&self) -> i32 {
        match self {
            Op::PushVar(_) | Op::PushConst(_) => 1,
            Op::BinOp(_) | Op::Cmp(_) => -1,
            Op::Not | Op::AvgHp => 0,
            Op::Roll => -1,
            Op::Sum
            | Op::Explode
            | Op::KeepMax(_)
            | Op::KeepMin(_)
            | Op::DropMax(_)
            | Op::DropMin(_) => 0,
            Op::AssignVar(_) => -1,
            Op::In => -2,
            Op::Eval(_) => 0,
            Op::EvalIf(_, _) => -1,
            Op::Each(_) | Op::Next => 1,
            Op::PushGroup(_, _) => 1,
            Op::AssignGroup(_, _) => -1,
            Op::Tier(n) => -(2 * *n as i32),
        }
    }

    fn compound_sym(&self) -> Option<&'static str> {
        match self {
            Op::BinOp(bin_op) => bin_op.compound_sym(),
            _ => None,
        }
    }
}

/// A single block of ops within an expression.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Block<Var, Val, Grp = NoGroup<Var>>(Box<[Op<Var, Val, Grp>]>);

impl<Var: PartialEq, Val: PartialEq, Grp: PartialEq> PartialEq<[Op<Var, Val, Grp>]>
    for Block<Var, Val, Grp>
{
    fn eq(&self, other: &[Op<Var, Val, Grp>]) -> bool {
        *self.0 == *other
    }
}

impl<Var, Val, Grp> std::ops::Deref for Block<Var, Val, Grp> {
    type Target = [Op<Var, Val, Grp>];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Var, Val, Grp> From<Vec<Op<Var, Val, Grp>>> for Block<Var, Val, Grp> {
    fn from(ops: Vec<Op<Var, Val, Grp>>) -> Self {
        Self(ops.into_boxed_slice())
    }
}

impl<Var, Val, Grp> Block<Var, Val, Grp> {
    /// Returns true if this block contains any variable matching the predicate.
    /// Group ops (`PushGroup`/`AssignGroup`) over-approximate as `true` — they
    /// reference a column of a runtime row whose `Var` can't be enumerated
    /// without the Context. Callers needing precision should walk the
    /// materialised rows via `Context::resolve_group`.
    pub fn has_var(&self, pred: &impl Fn(&Var) -> bool) -> bool {
        self.0.iter().any(|op| match op {
            Op::PushVar(v) | Op::AssignVar(v) => pred(v),
            Op::PushGroup(_, _) | Op::AssignGroup(_, _) => true,
            _ => false,
        })
    }

    /// Returns true if this block contains a dice roll (`Op::Roll`).
    pub fn has_dice(&self) -> bool {
        self.0.iter().any(|op| matches!(op, Op::Roll))
    }

    /// Returns true if this block assigns to any variable matching the
    /// predicate. Group assigns over-approximate as `true` (see
    /// [`Self::has_var`]).
    pub fn assigns_to(&self, pred: &impl Fn(&Var) -> bool) -> bool {
        self.0.iter().any(|op| match op {
            Op::AssignVar(v) => pred(v),
            Op::AssignGroup(_, _) => true,
            _ => false,
        })
    }

    /// Create a new block by mapping each op.
    pub fn map(&self, f: &mut impl FnMut(&Op<Var, Val, Grp>) -> Op<Var, Val, Grp>) -> Self {
        Self(self.0.iter().map(f).collect())
    }
}

impl<Var: PartialEq, Val, Grp> Block<Var, Val, Grp> {
    /// Split this block into statements at `Assign` boundaries.
    pub fn statements(&self) -> impl Iterator<Item = &[Op<Var, Val, Grp>]> {
        self.0
            .split_inclusive(|op| matches!(op, Op::AssignVar(_) | Op::AssignGroup(_, _)))
    }

    /// Detect compound assignment pattern in an ops slice (a single statement).
    ///
    /// Returns `Some(CompoundAssign)` if the ops form `[prefix] Push(X) <rhs>
    /// BinaryOp Assign(X)` — a compound assignment like `X += rhs`, optionally
    /// preceded by stack-balanced prefix ops (e.g. an inner each-loop with
    /// side effects). Works for both `PushVar`/`Assign` and
    /// `PushGroup`/`AssignGroup` pairs.
    ///
    /// `Op::Each + Op::EvalIf(_, BLOCK_NOOP)` pairs are treated as net 0 in
    /// the prefix (top-level `each` statements consume their pushed sentinel)
    /// and net +1 in the rhs (value-producing `fold(...)` accumulators).
    /// The combining BinOp must be the last body op AND the only 2→1 stack
    /// transition there — guarantees it consumes the initial Push(X) rather
    /// than an inner sub-expression.
    pub fn detect_compound(ops: &[Op<Var, Val, Grp>]) -> Option<CompoundAssign>
    where
        Grp: PartialEq,
    {
        if ops.len() < 3 {
            return None;
        }
        let assign_op = ops.last()?;
        let push_idx = find_compound_push(ops, assign_op)?;

        let body = &ops[push_idx + 1..ops.len() - 1];
        let last_body = body.len().checked_sub(1)?;
        let sym = body.last()?.compound_sym()?;

        let mut depth: i32 = 1;
        let mut i = 0;
        while i < body.len() {
            let (delta, advance) = rhs_step_delta(body, i);
            let new_depth = depth + delta;
            if advance == 1 && new_depth == 1 && depth == 2 {
                return (i == last_body).then_some(CompoundAssign {
                    sym,
                    prefix_end: push_idx,
                    rhs_start: push_idx + 1,
                    rhs_end: push_idx + 1 + i,
                });
            }
            depth = new_depth;
            i += advance;
        }
        None
    }
}

/// Find the first `Push(X)` matching the final `Assign(X)` at stack depth 0,
/// walking past any stack-balanced prefix. `Op::Each + Op::EvalIf(_, NOOP)`
/// pairs in the prefix are top-level `each(...)` statements (net 0).
fn find_compound_push<Var: PartialEq, Val, Grp: PartialEq>(
    ops: &[Op<Var, Val, Grp>],
    assign_op: &Op<Var, Val, Grp>,
) -> Option<usize> {
    let mut depth: i32 = 0;
    let mut i = 0;
    while i < ops.len() - 1 {
        let op = &ops[i];
        if depth == 0
            && match (op, assign_op) {
                (Op::PushVar(p), Op::AssignVar(a)) => p == a,
                (Op::PushGroup(p, pc), Op::AssignGroup(a, ac)) => p == a && pc == ac,
                _ => false,
            }
        {
            return Some(i);
        }
        if is_loop_pair(ops, i) {
            // each-statement at top level: pushes sentinel, EvalIf pops it,
            // body runs side-effects with net-0 stack contribution.
            i += 2;
            continue;
        }
        depth += op.stack_delta();
        i += 1;
    }
    None
}

/// One step of stack-depth walk inside a compound's rhs. Returns
/// `(delta, advance)` — how the stack changes and how many ops to skip.
/// `Op::Each + Op::EvalIf(_, NOOP)` in rhs context is a `fold(...)` value
/// (net +1, advance 2); everything else uses `Op::stack_delta`.
fn rhs_step_delta<Var: PartialEq, Val, Grp>(ops: &[Op<Var, Val, Grp>], i: usize) -> (i32, usize) {
    if is_loop_pair(ops, i) {
        (1, 2)
    } else {
        (ops[i].stack_delta(), 1)
    }
}

fn is_loop_pair<Var, Val, Grp>(ops: &[Op<Var, Val, Grp>], i: usize) -> bool {
    matches!(ops.get(i), Some(Op::Each(_)))
        && matches!(ops.get(i + 1), Some(Op::EvalIf(_, BLOCK_NOOP)))
}
