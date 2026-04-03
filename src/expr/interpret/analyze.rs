use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use super::eval_block;
use crate::expr::{
    Context, Expr, Op, avg_hp,
    ops::{BLOCK_MAIN, BlockIndex},
    stack::Stack,
};

/// Result of expression analysis: which ARGs are reachable and which dice are
/// needed.
#[derive(Debug, Clone, Default)]
pub struct ExprAnalysis {
    /// Dice requirements: die sides → total roll count.
    pub dice_rolls: BTreeMap<u32, u32>,
    /// ARG indices that appear in reachable (non-guarded-out) blocks.
    pub active_args: Vec<u8>,
    /// ARG indices constrained to 0/1 by `in(ARG.n, 0, 1)`.
    pub boolean_args: BTreeSet<u8>,
}

impl ExprAnalysis {
    /// Analyze an expression: run read-only, collect dice requirements and
    /// determine which ARG variables are in reachable blocks.
    ///
    /// `is_arg` inspects a variable and returns `Some(index)` if it is an ARG
    /// variable, `None` otherwise. ARGs resolve to 0 during analysis.
    ///
    /// For `guard(cond, body)` / `if(cond, then)`: if the condition is
    /// non-interactive (no ARGs) and evaluates to false, the then-block is
    /// skipped — its ARGs won't appear in `active_args`.
    pub fn analyze<Var, Ctx>(
        expr: &Expr<Var, i32>,
        ctx: &Ctx,
        is_arg: impl Fn(&Var) -> Option<u8> + Copy,
    ) -> Self
    where
        Var: Copy + fmt::Display,
        Ctx: Context<Var, i32>,
    {
        let mut analysis = Self::default();
        let _ = analysis.analyze_block(expr, ctx, is_arg, BLOCK_MAIN);
        analysis
    }

    fn analyze_block<Var, Ctx>(
        &mut self,
        expr: &Expr<Var, i32>,
        ctx: &Ctx,
        is_arg: impl Fn(&Var) -> Option<u8> + Copy,
        block: BlockIndex,
    ) -> AnalyzedBlock
    where
        Var: Copy + fmt::Display,
        Ctx: Context<Var, i32>,
    {
        let ops = expr.block(block);
        let mut stack = Stack::new();
        let mut has_args = false;
        let mut last_eval_had_args = false;
        // State machine for detecting `in(ARG.n, 0, 1)` patterns inline.
        // Tracks (arg_index, steps_matched): 1 = saw Arg, 2 = saw 0, 3 = saw 1.
        let mut bool_detect: Option<(u8, u8)> = None;

        for &op in ops.iter() {
            match op {
                Op::PushVar(var) => {
                    if let Some(idx) = is_arg(&var) {
                        has_args = true;
                        self.active_args.push(idx);
                        stack.push(0);
                    } else {
                        stack.push(ctx.resolve(var).unwrap_or(0));
                    }
                }
                Op::Assign(_) => {}
                Op::Roll => {
                    let (count, sides) = stack.pop2().unwrap_or((0, 0));
                    if count > 0 && sides > 0 {
                        *self.dice_rolls.entry(sides as u32).or_insert(0) += count as u32;
                    }
                    for _ in 0..count {
                        stack.push(avg_hp(sides));
                    }
                    stack.push(sides);
                    stack.push(count);
                }
                Op::Eval(idx) => {
                    if let Ok(Some(block_idx)) = eval_block(idx) {
                        // Eval blocks are conditions — their ARGs are for
                        // validation display, not input fields. Analyze but
                        // don't add their ARGs to active_args.
                        let saved_len = self.active_args.len();
                        let sub = self.analyze_block(expr, ctx, is_arg, block_idx);
                        self.active_args.truncate(saved_len);
                        last_eval_had_args = sub.has_args;
                        has_args |= sub.has_args;
                        stack.push(sub.result);
                    }
                }
                Op::EvalIf(then_idx, else_idx) => {
                    let cond = stack.pop().unwrap_or(0);
                    // Interactive condition (has ARGs) → visit both branches
                    // to discover all reachable ARGs. Non-interactive false
                    // condition → prune the then branch.
                    if cond != 0 || last_eval_had_args {
                        // Visit both branches for ARG discovery; push
                        // then-branch result for stack continuity.
                        let mut pushed = false;
                        for idx in [then_idx, else_idx] {
                            if let Ok(Some(block_idx)) = eval_block(idx) {
                                let sub = self.analyze_block(expr, ctx, is_arg, block_idx);
                                has_args |= sub.has_args;
                                if !pushed {
                                    stack.push(sub.result);
                                    pushed = true;
                                }
                            }
                        }
                    } else if let Ok(Some(block_idx)) = eval_block(else_idx) {
                        let sub = self.analyze_block(expr, ctx, is_arg, block_idx);
                        has_args |= sub.has_args;
                        stack.push(sub.result);
                    }
                    last_eval_had_args = false;
                }
                Op::In => {
                    if let Some((arg_idx, 3)) = bool_detect {
                        self.boolean_args.insert(arg_idx);
                    }
                    let _ = super::eval_op(&mut stack, op);
                }
                op => {
                    let _ = super::eval_op(&mut stack, op);
                }
            }

            // Advance boolean-arg pattern: Arg(n) → PushConst(0) → PushConst(1) → In
            bool_detect = match op {
                Op::PushVar(var) => is_arg(&var).map(|idx| (idx, 1)),
                Op::PushConst(0) if matches!(bool_detect, Some((_, 1))) => {
                    Some((bool_detect.unwrap().0, 2))
                }
                Op::PushConst(1) if matches!(bool_detect, Some((_, 2))) => {
                    Some((bool_detect.unwrap().0, 3))
                }
                _ => None,
            };
        }

        AnalyzedBlock {
            has_args,
            result: stack.result().unwrap_or(0),
        }
    }
}

struct AnalyzedBlock {
    has_args: bool,
    result: i32,
}
