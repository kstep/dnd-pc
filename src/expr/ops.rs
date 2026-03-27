use serde::{Deserialize, Serialize};

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
pub enum Op<Var, Val> {
    PushVar(Var),
    PushConst(Val),
    Add,      // +
    Sub,      // -
    Mul,      // *
    DivFloor, // /
    DivCeil,  // \
    Min,
    Max,
    AvgHp,
    Roll,         // 2d20 -> 2 20 Roll Sum
    KeepMax(Val), // 2d20kh1 -> 2 20 Roll KeepMax(1)
    KeepMin(Val),
    DropMax(Val),
    DropMin(Val),
    Sum,
    Explode, // Nd8! → roll sequentially, sum until a die rolls less than max (sides)
    Assign(Var),
    Mod, // %
    And, // logical and (0/1)
    Or,  // logical or (0/1)
    Not, // logical not (0/1)
    Cmp(Cmp),
    In,                             // in(a, b, c) → b <= a && a <= c
    EvalIf(BlockIndex, BlockIndex), // if: pop cond, branch to then/else block
    Eval(BlockIndex),               // evaluate sub-block
}

impl<Var: PartialEq, Val> Op<Var, Val> {
    /// Net stack-depth change of this op (+1 for push, -1 for binary, etc).
    fn stack_delta(&self) -> i32 {
        match self {
            Op::PushVar(_) | Op::PushConst(_) => 1,
            // Binary ops: pop 2, push 1 → -1
            Op::Add
            | Op::Sub
            | Op::Mul
            | Op::DivFloor
            | Op::DivCeil
            | Op::Mod
            | Op::Min
            | Op::Max
            | Op::And
            | Op::Or
            | Op::Cmp(_) => -1,
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
            Op::Assign(_) => -1,
            Op::In => -2, // pop 3, push 1
            Op::Eval(_) => 0,
            Op::EvalIf(_, _) => -1, // pops condition
        }
    }

    fn compound_sym(&self) -> Option<&'static str> {
        match self {
            Op::Add => Some("+"),
            Op::Sub => Some("-"),
            Op::Mul => Some("*"),
            Op::DivFloor => Some("/"),
            Op::DivCeil => Some("\\"),
            Op::Mod => Some("%"),
            _ => None,
        }
    }
}

/// A single block of ops within an expression.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Block<Var, Val>(Box<[Op<Var, Val>]>);

impl<Var: PartialEq, Val: PartialEq> PartialEq<[Op<Var, Val>]> for Block<Var, Val> {
    fn eq(&self, other: &[Op<Var, Val>]) -> bool {
        *self.0 == *other
    }
}

impl<Var, Val> std::ops::Deref for Block<Var, Val> {
    type Target = [Op<Var, Val>];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Var, Val> From<Vec<Op<Var, Val>>> for Block<Var, Val> {
    fn from(ops: Vec<Op<Var, Val>>) -> Self {
        Self(ops.into_boxed_slice())
    }
}

impl<Var, Val> Block<Var, Val> {
    /// Returns true if this block contains any variable matching the predicate.
    pub fn has_var(&self, pred: &impl Fn(&Var) -> bool) -> bool {
        self.0.iter().any(|op| match op {
            Op::PushVar(v) | Op::Assign(v) => pred(v),
            _ => false,
        })
    }

    /// Replace ops in-place by applying a mapping function to each op.
    pub fn replace(&mut self, mut f: impl FnMut(&Op<Var, Val>) -> Op<Var, Val>) {
        for op in self.0.iter_mut() {
            *op = f(op);
        }
    }
}

impl<Var: PartialEq, Val> Block<Var, Val> {
    /// Split this block into statements at `Assign` boundaries.
    pub fn statements(&self) -> impl Iterator<Item = &[Op<Var, Val>]> {
        self.0.split_inclusive(|op| matches!(op, Op::Assign(_)))
    }

    /// Detect compound assignment pattern in an ops slice (a single statement).
    ///
    /// Returns `Some(CompoundAssign)` if the ops form `PushVar(X) <rhs>
    /// BinaryOp Assign(X)` — i.e. a compound assignment like `X += rhs`.
    /// The combining op is identified by stack-depth analysis: it's the first
    /// binary op that would consume the initial variable from the stack.
    pub fn detect_compound(ops: &[Op<Var, Val>]) -> Option<CompoundAssign> {
        if ops.len() < 3 {
            return None;
        }
        let Op::Assign(assign_var) = &ops[ops.len() - 1] else {
            return None;
        };
        let Op::PushVar(push_var) = &ops[0] else {
            return None;
        };
        if push_var != assign_var {
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
