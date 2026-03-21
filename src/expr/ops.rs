use serde::{Deserialize, Serialize};

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
    Assign(Var),
    Mod,   // %
    And,   // logical and (0/1)
    Or,    // logical or (0/1)
    Not,   // logical not (0/1)
    Lt,    // <
    Gt,    // >
    Le,    // <=
    Ge,    // >=
    CmpEq, // ==
    CmpNe, // !=
    If,      // if(cond, then, else)
    Eval(u8), // noop placeholder
}
