use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::expr::{
    Context, Expr, Op, VarGroup, avg_hp,
    group::IterStack,
    interpret::eval_block,
    ops::{BLOCK_ERROR, BLOCK_MAIN, BlockIndex},
    stack::Stack,
};

/// Result of expression analysis: which ARGs are reachable and which dice are
/// needed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExprAnalysis {
    /// Dice requirements: die sides → total roll count.
    pub dice_rolls: BTreeMap<u32, u32>,
    /// ARG indices that appear in reachable (non-guarded-out) blocks.
    pub active_args: BTreeSet<u8>,
    /// ARG indices constrained to 0/1 by `in(ARG.n, 0, 1)`.
    pub boolean_args: BTreeSet<u8>,
    /// True if the expression contains a `guard(...)` — i.e. an
    /// `Op::EvalIf(_, BLOCK_ERROR)` — which enforces cardinality:
    /// `eval_lenient` returns Err until the guard's condition is satisfied.
    /// Expressions without a guard are trivially always "valid", so the UI
    /// can't use that signal to decide when to disable untouched inputs.
    pub has_guard: bool,
}

impl ExprAnalysis {
    /// Analyze an expression: run read-only, collect dice requirements and
    /// determine which ARG variables are in reachable blocks.
    ///
    /// `arg_index` inspects a variable and returns `Some(index)` if it is an
    /// ARG variable, `None` otherwise. ARGs resolve to 0 during analysis.
    ///
    /// For `guard(cond, body)` / `if(cond, then)`: if the condition is
    /// non-interactive (no ARGs) and evaluates to false, the then-block is
    /// skipped — its ARGs won't appear in `active_args`.
    pub fn analyze<Var, Ctx, Grp>(
        expr: &Expr<Var, i32, Grp>,
        ctx: &Ctx,
        arg_index: impl Fn(&Var) -> Option<u8> + Copy,
    ) -> Self
    where
        Var: Copy + fmt::Display,
        Grp: Copy + VarGroup<Var = Var>,
        Ctx: Context<Var, i32>,
    {
        let mut analysis = Self::default();
        let mut iter_stack = IterStack::new();
        let _ = analysis.analyze_block(expr, ctx, arg_index, BLOCK_MAIN, &mut iter_stack, false);
        analysis
    }

    fn analyze_block<Var, Ctx, Grp>(
        &mut self,
        expr: &Expr<Var, i32, Grp>,
        ctx: &Ctx,
        arg_index: impl Fn(&Var) -> Option<u8> + Copy,
        block: BlockIndex,
        iter_stack: &mut IterStack,
        suppress_args: bool,
    ) -> AnalyzedBlock
    where
        Var: Copy + fmt::Display,
        Grp: Copy + VarGroup<Var = Var>,
        Ctx: Context<Var, i32>,
    {
        let ops = expr.block(block);
        let mut stack = Stack::new();
        let mut has_args = false;
        let mut last_eval_had_args = false;
        // State machine for detecting `in(ARG.n, 0, 1)` patterns inline.
        // Tracks (arg_index, steps_matched): 1 = saw Arg, 2 = saw 0, 3 = saw 1.
        let mut bool_detect: Option<(u8, u8)> = None;

        let mut ops_iter = ops.iter().copied().peekable();
        while let Some(op) = ops_iter.next() {
            match op {
                Op::PushVar(var) => {
                    if let Some(idx) = arg_index(&var) {
                        has_args = true;
                        if !suppress_args {
                            self.active_args.insert(idx);
                        }
                        stack.push(0);
                    } else {
                        stack.push(ctx.resolve(var).unwrap_or(0));
                    }
                }
                Op::AssignVar(_) | Op::AssignGroup(_) => {}
                Op::PushGroup(grp) => {
                    if let Some(var) = grp.top_member(iter_stack) {
                        if let Some(arg_idx) = arg_index(&var) {
                            has_args = true;
                            if !suppress_args {
                                self.active_args.insert(arg_idx);
                            }
                            stack.push(0);
                        } else {
                            stack.push(ctx.resolve(var).unwrap_or(0));
                        }
                    } else {
                        stack.push(0);
                    }
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
                        // Parser emits Op::Eval in two shapes (see parser.rs:
                        // guard→411-422, if_expr→429-433, fold→543-548):
                        // 1) guard/if cond: Op::Eval(cond) + Op::EvalIf(then, else) — ARGs inside
                        //    cond are plumbing, not user-facing.
                        // 2) fold accumulator body: Op::Eval(body) + Op::BinOp — ARGs inside body
                        //    ARE user-facing (e.g. the @ARG being folded).
                        // Peek the next op: EvalIf → cond (suppress ARG collection),
                        // otherwise → body (keep). Breaks silently if parser ever
                        // inserts an op between the Eval and EvalIf of a cond.
                        let is_cond = matches!(ops_iter.peek(), Some(Op::EvalIf(_, _)));
                        let sub = self.analyze_block(
                            expr,
                            ctx,
                            arg_index,
                            block_idx,
                            iter_stack,
                            suppress_args || is_cond,
                        );
                        last_eval_had_args = sub.has_args;
                        has_args |= sub.has_args;
                        stack.push(sub.result);
                    }
                }
                Op::EvalIf(then_idx, else_idx) => {
                    self.has_guard |= else_idx == BLOCK_ERROR;
                    let cond = stack.pop().unwrap_or(0);
                    if cond != 0 || last_eval_had_args {
                        let mut pushed = false;
                        for idx in [then_idx, else_idx] {
                            if let Ok(Some(block_idx)) = eval_block(idx) {
                                let sub = self.analyze_block(
                                    expr,
                                    ctx,
                                    arg_index,
                                    block_idx,
                                    iter_stack,
                                    suppress_args,
                                );
                                has_args |= sub.has_args;
                                if !pushed {
                                    stack.push(sub.result);
                                    pushed = true;
                                }
                            }
                        }
                    } else if let Ok(Some(block_idx)) = eval_block(else_idx) {
                        let sub = self.analyze_block(
                            expr,
                            ctx,
                            arg_index,
                            block_idx,
                            iter_stack,
                            suppress_args,
                        );
                        has_args |= sub.has_args;
                        stack.push(sub.result);
                    }
                    last_eval_had_args = false;
                }
                Op::In => {
                    if let Some((arg_idx, 3)) = bool_detect {
                        self.boolean_args.insert(arg_idx);
                    }
                    let _ = super::eval_op(&mut stack, iter_stack, op);
                }
                op => {
                    let _ = super::eval_op(&mut stack, iter_stack, op);
                }
            }

            // Advance boolean-arg pattern: Arg(n) → PushConst(0) → PushConst(1) → In
            bool_detect = match op {
                Op::PushVar(var) => arg_index(&var).map(|idx| (idx, 1)),
                Op::PushGroup(grp) => grp
                    .top_member(iter_stack)
                    .and_then(|var| arg_index(&var))
                    .map(|idx| (idx, 1)),
                Op::PushConst(0) => match bool_detect {
                    Some((idx, 1)) => Some((idx, 2)),
                    _ => None,
                },
                Op::PushConst(1) => match bool_detect {
                    Some((idx, 2)) => Some((idx, 3)),
                    _ => None,
                },
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::model::{Attribute, Character, ProficiencyLevel, Skill};

    fn arg_index(var: &Attribute) -> Option<u8> {
        if let Attribute::Arg(n) = var {
            Some(*n)
        } else {
            None
        }
    }

    fn character_with_skills(levels: &[(Skill, ProficiencyLevel)]) -> Character {
        // Avoid Character::default() — it calls js_sys::Date::now() and
        // Uuid::new_v4(), both of which panic outside WASM. Build from a
        // minimal JSON skeleton instead.
        let mut ch: Character = serde_json::from_str(
            r#"{"id":"00000000-0000-0000-0000-000000000000","schema_version":0}"#,
        )
        .expect("minimal character JSON");
        for (skill, level) in levels {
            ch.skills.set(*skill, *level);
        }
        ch
    }

    #[test]
    fn analyze_loop_per_iter_filters_by_current_value() {
        // Skill index→variant: Acrobatics=0, AnimalHandling=1, Arcana=2,
        // Athletics=3. Setup below sets Acrobatics and Athletics to
        // Proficient, Arcana to Expertise; AnimalHandling stays at default
        // None. The expression filters on `@ == 1` (Proficient only), so
        // Expertise at Arcana and None at AnimalHandling are excluded —
        // only Acrobatics (0) and Athletics (3) remain in active_args.
        let character = character_with_skills(&[
            (Skill::Acrobatics, ProficiencyLevel::Proficient),
            (Skill::Athletics, ProficiencyLevel::Proficient),
            (Skill::Arcana, ProficiencyLevel::Expertise),
        ]);

        let expr: crate::model::Expr = "with(@SKILL._.PROF, each(@, if(@ == 1, @ += @ARG)))"
            .parse()
            .unwrap();

        let analysis = expr.analyze(&character, arg_index);

        assert_eq!(
            analysis.active_args,
            BTreeSet::from([0, 3]),
            "expected only Proficient skills (Acrobatics=0, Athletics=3), got {:?}",
            analysis.active_args
        );
    }

    #[test]
    fn analyze_loop_in_pattern_marks_each_iter_arg_boolean() {
        // fold(and, @GROUP, in(@ARG, 0, 1)) should mark every iter_no in the
        // group as a boolean ARG — matching the `in(...)` pattern inside the
        // loop body, resolved per iter.
        let character = character_with_skills(&[
            (Skill::Acrobatics, ProficiencyLevel::Proficient),
            (Skill::Athletics, ProficiencyLevel::Proficient),
        ]);
        let expr: crate::model::Expr =
            "with(@SKILL._.PROF, guard(fold(and, @, in(@ARG, 0, 1)) and fold(+, @, @ARG) == 2, each(@, if(@ == 1, @ += @ARG))))"
                .parse()
                .expect("expr must parse");
        let analysis = expr.analyze(&character, arg_index);
        // Both active iter_indices (0 and 3) must be marked boolean.
        assert!(analysis.boolean_args.contains(&0), "iter 0 must be boolean");
        assert!(analysis.boolean_args.contains(&3), "iter 3 must be boolean");
    }

    #[test]
    fn analyze_loop_no_filter_preserves_all_indices() {
        // Generation: Custom uses `each(@ABILITY, @ = @ARG)` — no filter.
        // All 6 abilities must stay active after our refactor.
        let character = character_with_skills(&[]);
        let expr: crate::model::Expr = "with(@ABILITY, each(@, @ = @ARG))"
            .parse()
            .expect("expr must parse");
        let analysis = expr.analyze(&character, arg_index);
        assert_eq!(
            analysis.active_args,
            BTreeSet::from([0, 1, 2, 3, 4, 5]),
            "all 6 abilities must remain active"
        );
    }

    #[test]
    fn analyze_loop_with_outer_mask() {
        // Masked subgroup (@ABILITY(STR, DEX)) × per-iter filter @ < 20.
        // A freshly-built character has all abilities below 20, so both
        // STR and DEX iters should end up active (iter_no 0 and 1 within
        // the masked subgroup).
        let character = character_with_skills(&[]);
        let expr: crate::model::Expr = "with(@ABILITY(STR, DEX), each(@, if(@ < 20, @ += @ARG)))"
            .parse()
            .expect("expr must parse");
        let analysis = expr.analyze(&character, arg_index);
        assert_eq!(
            analysis.active_args,
            BTreeSet::from([0, 1]),
            "both STR and DEX iters must be active"
        );
    }

    #[test]
    fn analyze_loop_collects_dice_per_active_iter() {
        // Body `if(@ == 1, 1d6 * @ARG)` collects one d6 per iter where
        // @ == 1 is true. Two Proficient skills → two d6 rolls.
        let character = character_with_skills(&[
            (Skill::Acrobatics, ProficiencyLevel::Proficient),
            (Skill::Athletics, ProficiencyLevel::Proficient),
        ]);
        let expr: crate::model::Expr = "with(@SKILL._.PROF, each(@, if(@ == 1, @ += 1d6 * @ARG)))"
            .parse()
            .expect("expr must parse");
        let analysis = expr.analyze(&character, arg_index);
        assert_eq!(
            analysis.dice_rolls.get(&6).copied(),
            Some(2),
            "d6 count must equal number of active iters"
        );
    }

    #[test]
    fn analyze_nested_each_collects_inner_iter_args() {
        // Nested `each(@A, each(@B, … @ARG))`: `@ARG` is a companion group
        // that resolves through `iter_stack.top()`, i.e. the INNER loop's
        // current iter_no. Across all outer×inner iterations, `active_args`
        // should cover every inner iter_no exactly once (BTreeSet dedupes).
        let character = character_with_skills(&[]);
        let expr: crate::model::Expr = "each(@ABILITY, each(@ABILITY, @ABILITY += @ARG))"
            .parse()
            .expect("expr must parse");
        let analysis = expr.analyze(&character, arg_index);
        assert_eq!(
            analysis.active_args,
            BTreeSet::from([0, 1, 2, 3, 4, 5]),
            "inner @ARG iter indices must all be active"
        );
    }

    #[test]
    fn analyze_guard_static_false_prunes_loop_args() {
        // A non-interactive guard whose condition statically evaluates to
        // false suppresses the entire body — including any loop and its
        // ARGs. AC on a default-built character is 0, so `AC >= 999` is
        // trivially false without any ARG in the cond.
        let character = character_with_skills(&[]);
        let expr: crate::model::Expr = "guard(AC >= 999, each(@ABILITY, @ABILITY += @ARG))"
            .parse()
            .expect("expr must parse");
        let analysis = expr.analyze(&character, arg_index);
        assert!(
            analysis.active_args.is_empty(),
            "statically-false guard must suppress all loop args, got {:?}",
            analysis.active_args
        );
    }

    #[test]
    fn analyze_standalone_fold_collects_all_iter_args() {
        // Standalone `fold(op, @G, @ARG)` without an outer `each` must still
        // mark every iter_idx of @G as active. Regression: Op::Eval in the
        // fold accumulator block `mem::take`s active_args (the correct
        // behaviour for guard/if condition evaluation), which wrongly
        // discards ARGs pushed by the fold body on every iteration except
        // the first.
        let character = character_with_skills(&[]);
        let expr: crate::model::Expr = "fold(+, @ABILITY, @ARG)".parse().expect("expr must parse");
        let analysis = expr.analyze(&character, arg_index);
        assert_eq!(
            analysis.active_args,
            BTreeSet::from([0, 1, 2, 3, 4, 5]),
            "standalone fold must mark all 6 ABILITY iter_indices as active, got {:?}",
            analysis.active_args
        );
    }

    #[test]
    fn analyze_has_guard_true_for_guard_expression() {
        // `guard(cond, body)` compiles to `Op::EvalIf(_, BLOCK_ERROR)`.
        let character = character_with_skills(&[]);
        let expr: crate::model::Expr = "guard(AC >= 10, AC += ARG.0)"
            .parse()
            .expect("expr must parse");
        let analysis = expr.analyze(&character, arg_index);
        assert!(analysis.has_guard, "guard expression must set has_guard");
    }

    #[test]
    fn analyze_has_guard_false_for_no_guard_expression() {
        // Plain `each` / `fold` without a guard should not set has_guard.
        // Generation: Custom is the driving case: all ability inputs must
        // stay editable (is_satisfied in ExprArgsInput gates on has_guard).
        let character = character_with_skills(&[]);
        let expr: crate::model::Expr = "each(@ABILITY, @ABILITY = @ARG)"
            .parse()
            .expect("expr must parse");
        let analysis = expr.analyze(&character, arg_index);
        assert!(
            !analysis.has_guard,
            "no-guard expression must leave has_guard=false"
        );
    }

    #[test]
    fn analyze_has_guard_false_for_if_without_else() {
        // `if(cond, then)` compiles to `Op::EvalIf(then, BLOCK_NOOP)`, not
        // BLOCK_ERROR — not a guard. has_guard must remain false.
        let character = character_with_skills(&[]);
        let expr: crate::model::Expr = "if(AC >= 10, AC += ARG.0)"
            .parse()
            .expect("expr must parse");
        let analysis = expr.analyze(&character, arg_index);
        assert!(
            !analysis.has_guard,
            "if without else must not set has_guard"
        );
    }
}
