use serde::{Deserialize, Serialize};

use crate::expr::{
    Error,
    group::{NoGroup, VarGroup, VarSubgroup},
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
    /// Start index of the RHS ops (after the initial PushVar, exclusive).
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
    Each(VarSubgroup<Grp>),         // init loop: push real index to iter_stack, push 1/0
    Next(VarSubgroup<Grp>),         // advance to next member, push 1 if more, else pop+push 0
    PushGroup(Grp),                 // push group[iter_stack.last()] via ctx.resolve
    AssignGroup(Grp),               /* pop value, assign to group[iter_stack.last()] via
                                     * ctx.assign */
    Tier(u8), // pop var, pop N*(threshold,value) pairs, push matching value
}

impl<Var: PartialEq, Val, Grp> Op<Var, Val, Grp> {
    /// Net stack-depth change of this op (+1 for push, -1 for binary, etc).
    fn stack_delta(&self) -> i32 {
        match self {
            Op::PushVar(_) | Op::PushConst(_) => 1,
            Op::BinOp(_) | Op::Cmp(_) => -1,
            // Unary ops: pop 1, push 1 → 0
            Op::Not | Op::AvgHp => 0,
            // Roll: pop 2 (count, sides), push count+1 items → variable, but
            // always followed by Sum/Keep/Drop that reduces back. For compound
            // detection purposes, Roll+reducer is net -1 (like a binary op).
            // We won't encounter Roll in typical assignment expressions, so
            // treat it conservatively.
            Op::Roll => 0,
            Op::Sum
            | Op::Explode
            | Op::KeepMax(_)
            | Op::KeepMin(_)
            | Op::DropMax(_)
            | Op::DropMin(_) => 0,
            Op::AssignVar(_) => -1,
            Op::In => -2, // pop 3, push 1
            Op::Eval(_) => 0,
            Op::EvalIf(_, _) => -1,         // pops condition
            Op::Each(_) | Op::Next(_) => 1, // push boolean
            Op::PushGroup(_) => 1,
            Op::AssignGroup(_) => -1,
            Op::Tier(_) => 0, // variable net: -(1 + 2*n) + 1; approximated for compound detection
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
    pub fn has_var(&self, pred: &impl Fn(&Var) -> bool) -> bool
    where
        Grp: VarGroup<Var = Var>,
    {
        self.0.iter().any(|op| match op {
            Op::PushVar(v) | Op::AssignVar(v) => pred(v),
            Op::PushGroup(g) | Op::AssignGroup(g) => {
                (0..32).any(|i| g.member(i).is_some_and(|v| pred(&v)))
            }
            _ => false,
        })
    }

    /// Returns true if this block assigns to any variable matching the
    /// predicate.
    pub fn assigns_to(&self, pred: &impl Fn(&Var) -> bool) -> bool
    where
        Grp: VarGroup<Var = Var>,
    {
        self.0.iter().any(|op| match op {
            Op::AssignVar(v) => pred(v),
            Op::AssignGroup(g) => (0..32).any(|i| g.member(i).is_some_and(|v| pred(&v))),
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
            .split_inclusive(|op| matches!(op, Op::AssignVar(_) | Op::AssignGroup(_)))
    }

    /// Detect compound assignment pattern in an ops slice (a single statement).
    ///
    /// Returns `Some(CompoundAssign)` if the ops form `Push(X) <rhs>
    /// BinaryOp Assign(X)` — i.e. a compound assignment like `X += rhs`.
    /// Works for both `PushVar`/`Assign` and `PushGroup`/`AssignGroup` pairs.
    /// The combining op is identified by stack-depth analysis: it's the first
    /// binary op that would consume the initial variable from the stack.
    pub fn detect_compound(ops: &[Op<Var, Val, Grp>]) -> Option<CompoundAssign>
    where
        Grp: PartialEq,
    {
        if ops.len() < 3 {
            return None;
        }
        let is_matching_pair = match (&ops[0], &ops[ops.len() - 1]) {
            (Op::PushVar(push_var), Op::AssignVar(assign_var)) => push_var == assign_var,
            (Op::PushGroup(push_grp), Op::AssignGroup(assign_grp)) => push_grp == assign_grp,
            _ => false,
        };
        if !is_matching_pair {
            return None;
        }

        // Walk ops between PushVar and Assign, tracking stack depth (starts
        // at 1 because PushVar already pushed). The combining op is the last
        // op in the body AND must be the first binary op that reduces depth
        // from 2→1 (consuming the initial variable). If it's not the last op,
        // there are post-combine operations that prevent compound rendering.
        let body = &ops[1..ops.len() - 1]; // exclude PushVar and Assign
        let last_body = body.len().checked_sub(1)?;
        let sym = body.last()?.compound_sym()?;

        // Verify the last op is indeed the combining op via stack depth.
        let mut depth: i32 = 1;
        for (i, op) in body.iter().enumerate() {
            let new_depth = depth + op.stack_delta();
            if new_depth == 1 && depth == 2 {
                // First depth 2→1 transition — must be at the last position.
                return (i == last_body).then_some(CompoundAssign {
                    sym,
                    rhs_start: 1,
                    rhs_end: 1 + i,
                });
            }
            depth = new_depth;
        }
        None
    }
}
