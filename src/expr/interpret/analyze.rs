use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use super::eval_block;
use crate::expr::{
    Context, Expr, Op, VarGroup, avg_hp,
    group::{IterStack, VarSubgroup},
    ops::{BLOCK_MAIN, BLOCK_NOOP, BlockIndex},
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
    pub fn analyze<Var, Ctx, Grp>(
        expr: &Expr<Var, i32, Grp>,
        ctx: &Ctx,
        is_arg: impl Fn(&Var) -> Option<u8> + Copy,
    ) -> Self
    where
        Var: Copy + fmt::Display,
        Grp: Copy + VarGroup<Var = Var>,
        Ctx: Context<Var, i32>,
    {
        let mut analysis = Self::default();
        let _ = analysis.analyze_block(expr, ctx, is_arg, BLOCK_MAIN);
        analysis
    }

    fn analyze_block<Var, Ctx, Grp>(
        &mut self,
        expr: &Expr<Var, i32, Grp>,
        ctx: &Ctx,
        is_arg: impl Fn(&Var) -> Option<u8> + Copy,
        block: BlockIndex,
    ) -> AnalyzedBlock
    where
        Var: Copy + fmt::Display,
        Grp: Copy + VarGroup<Var = Var>,
        Ctx: Context<Var, i32>,
    {
        let ops = expr.block(block);
        let mut stack = Stack::new();
        let mut iter_stack = IterStack::new();
        let mut has_args = false;
        let mut last_eval_had_args = false;
        // State machine for detecting `in(ARG.n, 0, 1)` patterns inline.
        // Tracks (arg_index, steps_matched): 1 = saw Arg, 2 = saw 0, 3 = saw 1.
        let mut bool_detect: Option<(u8, u8)> = None;

        let mut i = 0;
        let ops_slice = &**ops;
        while i < ops_slice.len() {
            let op = ops_slice[i];
            match op {
                Op::Each(grp) if i + 1 < ops_slice.len() => {
                    if let Op::EvalIf(body_idx, BLOCK_NOOP) = ops_slice[i + 1] {
                        // Loop pattern detected. Analyze body as template.
                        self.analyze_loop_body(expr, is_arg, grp, body_idx);
                        has_args = true;
                        // Each pushes a boolean; EvalIf consumes it. Net: 0.
                        i += 2;
                        continue;
                    }
                    stack.push(1); // Each pushes truthy
                }
                Op::PushVar(var) => {
                    if let Some(idx) = is_arg(&var) {
                        has_args = true;
                        self.active_args.push(idx);
                        stack.push(0);
                    } else {
                        stack.push(ctx.resolve(var).unwrap_or(0));
                    }
                }
                Op::AssignVar(_) | Op::AssignGroup(_) => {}
                Op::Next(_) => {
                    stack.push(0); // Next pushes falsy (loop done in analysis)
                }
                Op::PushGroup(_) => {
                    stack.push(0);
                }
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
                    if cond != 0 || last_eval_had_args {
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
                    let _ = super::eval_op(&mut stack, &mut iter_stack, op);
                }
                op => {
                    let _ = super::eval_op(&mut stack, &mut iter_stack, op);
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

            i += 1;
        }

        AnalyzedBlock {
            has_args,
            result: stack.result().unwrap_or(0),
        }
    }

    /// Analyze a loop body as a template: determine group size and generate
    /// active_args. Detects `PushGroup(Arg)` → generates ARG indices 0..N.
    /// Detects `in($ARG, 0, 1)` → marks all as boolean.
    fn analyze_loop_body<Var, Grp>(
        &mut self,
        expr: &Expr<Var, i32, Grp>,
        is_arg: impl Fn(&Var) -> Option<u8> + Copy,
        subgrp: VarSubgroup<Grp>,
        body_idx: BlockIndex,
    ) where
        Var: Copy + fmt::Display,
        Grp: Copy + VarGroup<Var = Var>,
    {
        let mut has_arg_group = false;
        let mut has_in_pattern = false;
        let mut visited = BTreeSet::new();
        Self::scan_block_for_args(
            expr,
            &is_arg,
            body_idx,
            &mut has_arg_group,
            &mut has_in_pattern,
            &mut visited,
        );

        if has_arg_group {
            for (iter_no, _real_idx) in subgrp.real_indices().enumerate() {
                self.active_args.push(iter_no as u8);
                if has_in_pattern {
                    self.boolean_args.insert(iter_no as u8);
                }
            }
        }
    }

    /// Recursively scan a block and all reachable sub-blocks for PushGroup(Arg)
    /// and `in($ARG, 0, 1)` patterns.
    fn scan_block_for_args<Var, Grp>(
        expr: &Expr<Var, i32, Grp>,
        is_arg: &impl Fn(&Var) -> Option<u8>,
        block_idx: BlockIndex,
        has_arg_group: &mut bool,
        has_in_pattern: &mut bool,
        visited: &mut BTreeSet<BlockIndex>,
    ) where
        Var: Copy,
        Grp: Copy + VarGroup<Var = Var>,
    {
        if !visited.insert(block_idx) {
            return;
        }
        let block = expr.block(block_idx);
        let ops = &**block;
        for (i, &op) in ops.iter().enumerate() {
            match op {
                Op::PushGroup(g) => {
                    if let Some(var) = g.member(0.into())
                        && is_arg(&var).is_some()
                    {
                        *has_arg_group = true;
                        if i + 3 < ops.len()
                            && matches!(ops[i + 1], Op::PushConst(0))
                            && matches!(ops[i + 2], Op::PushConst(1))
                            && matches!(ops[i + 3], Op::In)
                        {
                            *has_in_pattern = true;
                        }
                    }
                }
                Op::Eval(idx) => {
                    if let Ok(Some(sub_idx)) = eval_block(idx) {
                        Self::scan_block_for_args(
                            expr,
                            is_arg,
                            sub_idx,
                            has_arg_group,
                            has_in_pattern,
                            visited,
                        );
                    }
                }
                Op::EvalIf(then_idx, else_idx) => {
                    for idx in [then_idx, else_idx] {
                        if let Ok(Some(sub_idx)) = eval_block(idx) {
                            Self::scan_block_for_args(
                                expr,
                                is_arg,
                                sub_idx,
                                has_arg_group,
                                has_in_pattern,
                                visited,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

struct AnalyzedBlock {
    has_args: bool,
    result: i32,
}
