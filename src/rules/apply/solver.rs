use std::iter::once;

use crate::{
    expr::{Context, Op, VarSubgroup},
    model::{AssignInputs, Attribute, AttributeGroup, Character, Expr},
    rules::{
        WhenCondition,
        apply::{args_ctx::WithArgsRef, pending::PendingFeature},
        feature::FeatureDefinition,
    },
};

/// Solver budget — total candidate evaluations allowed across the full
/// backtracking tree. Prevents adversarial feat combinations from hanging
/// the Rebuild button indefinitely. Single global cap; `enumerate_assign`
/// does not impose a per-feat limit (Rogue CP has 330 legal splits, a lower
/// per-feat cap would silently miss valid candidates in the tail).
const MAX_TOTAL_ATTEMPTS: usize = 5_000;

/// Interactive assignment within a feature — one per `@ARG`-using entry in
/// `FeatureDefinition::assign`. `expr` is an owned clone (cheap — `Expr`
/// is `Arc`-backed) so solver can mutate sibling fields without a lifetime
/// borrow conflict on recursive calls into `&mut feats`.
///
/// `forced` carries pre-determined args (from stored `character.features`)
/// — when set, `enumerate_assign` yields only that single candidate so the
/// solver applies it unchanged. Needed to keep stored-input features in the
/// same pipeline-order walk as unsolved ones (otherwise `Expertise`-style
/// features that depend on an earlier unsolved feat's output would see a
/// stale baseline).
pub struct AssignData {
    pub expr: Expr,
    pub mask: VarSubgroup<AttributeGroup>,
    pub arg_range: (i32, i32),
    pub args: Vec<i32>,
    pub forced: Option<Vec<i32>>,
}

/// One unsolved pending feature: its definition + pending info + the list
/// of its interactive assigns. `solve_feats` fills each assign's `args`
/// through backtracking; the caller emits `Vec<AssignInputs>` aligned with
/// assigns and looks up `pending` for level/source when building the final
/// `PendingInputs`.
pub struct FeatState<'a> {
    pub def: &'a FeatureDefinition,
    pub pending: &'a PendingFeature,
    pub assigns: Vec<AssignData>,
}

/// Scan expression ops for `in(@ARG_or_ArgGroup, lo, hi)` and return the
/// first `(lo, hi)` found. Bounds integer-arg enumeration; absent pattern
/// means "no declared bound" (caller uses a permissive default).
pub fn scan_arg_range(expr: &Expr) -> Option<(i32, i32)> {
    for block in expr.blocks() {
        for window in block.windows(4) {
            let [push, lo, hi, in_op] = window else {
                continue;
            };
            let arg_pushed = matches!(
                push,
                Op::PushVar(Attribute::Arg(_)) | Op::PushGroup(AttributeGroup::Arg)
            );
            if !arg_pushed || !matches!(in_op, Op::In) {
                continue;
            }
            let (Op::PushConst(lo_val), Op::PushConst(hi_val)) = (lo, hi) else {
                continue;
            };
            return Some((*lo_val, *hi_val));
        }
    }
    None
}

/// Find the outer `with(@X, ...)` binding — first non-`Arg` `Op::Each`.
pub fn outer_group(expr: &Expr) -> Option<VarSubgroup<AttributeGroup>> {
    expr.ops().find_map(|op| match op {
        Op::Each(sg) if sg.inner != AttributeGroup::Arg => Some(*sg),
        _ => None,
    })
}

fn passes_guard(expr: &Expr, baseline: &Character, args: &[i32]) -> bool {
    let ctx = WithArgsRef {
        inner: baseline,
        args,
    };
    expr.eval_lenient(&ctx).is_ok()
}

/// Lazy enumerator of arg-vector candidates for one assign.
///
/// Yields, in priority order:
/// 1. Diff-exact — each active slot gets `target - baseline` clamped to
///    `arg_range`.
/// 2. Zero — all zeros (no-op candidate).
/// 3. Brute-force over active slots, bounded by `MAX_TOTAL_ATTEMPTS` at
///    construction time so the range can't explode to `3^18`.
///
/// "Active" = positions where `target` differs from `baseline` in either
/// direction; `arg_range` and `eval_lenient` filter impossible combinations.
fn enumerate_assign<'a>(
    mask: VarSubgroup<AttributeGroup>,
    arg_range: (i32, i32),
    arg_count: usize,
    forced: Option<&'a [i32]>,
    baseline: &'a Character,
    target: &'a Character,
) -> Box<dyn Iterator<Item = Vec<i32>> + 'a> {
    if let Some(forced_args) = forced {
        return Box::new(once(forced_args.to_vec()));
    }
    let (lo, hi) = arg_range;

    // Active slots = any position where target differs from baseline.
    // Both signs included — a future trade-off feature (negative @ARG range)
    // may need to decrement one slot to credit another. `eval_lenient` filters
    // candidates whose guards forbid the value, and `arg_range` clamps any
    // diff that can't be expressed within the feat's allowed args.
    //
    // Single-pass: build `positions` and fill `diff_exact` together.
    let mut diff_exact = vec![0i32; arg_count];
    let mut positions: Vec<usize> = Vec::new();
    for (pos, slot) in mask.members().enumerate() {
        let diff = target.resolve(slot).unwrap_or(0) - baseline.resolve(slot).unwrap_or(0);
        if diff != 0 {
            diff_exact[pos] = diff.clamp(lo, hi);
            positions.push(pos);
        }
    }
    let zero = vec![0i32; arg_count];

    let values_per_slot = (hi - lo + 1).max(1) as usize;
    let brute_count = values_per_slot
        .checked_pow(positions.len() as u32)
        .unwrap_or(usize::MAX)
        .min(MAX_TOTAL_ATTEMPTS);

    let brute = (0..brute_count).map(move |enc| {
        let mut args = vec![0i32; arg_count];
        let mut remaining = enc;
        for &pos in &positions {
            args[pos] = lo + (remaining as i32 % values_per_slot as i32);
            remaining /= values_per_slot;
        }
        args
    });

    let diff_exact_copy = diff_exact.clone();
    let zero_copy = zero.clone();
    Box::new(
        once(diff_exact)
            .chain(once(zero))
            .chain(brute.filter(move |args| *args != diff_exact_copy && *args != zero_copy)),
    )
}

/// Recursively solve assigns of `feats[feat_idx]` starting from assign_idx.
/// When all of this feat's assigns are filled, apply the whole feat to a
/// baseline clone and recurse to the next feat. On deeper failure the
/// current candidate is rolled back and the next is tried. `attempts`
/// decrements once per candidate tried; when it reaches zero the solver
/// bails with `false` (budget cap — prevents adversarial feat combinations
/// from hanging the Rebuild button).
fn solve_assign(
    feats: &mut [FeatState<'_>],
    feat_idx: usize,
    assign_idx: usize,
    baseline: &Character,
    target: &Character,
    attempts: &mut usize,
) -> bool {
    if feat_idx >= feats.len() {
        return baseline.eq_derived(target);
    }
    if assign_idx >= feats[feat_idx].assigns.len() {
        // All assigns solved — apply the feat and recurse to next.
        let mut trial = baseline.clone_lean();
        let inputs: Vec<AssignInputs> = feats[feat_idx]
            .assigns
            .iter()
            .map(|assign| AssignInputs {
                args: assign.args.clone(),
                ..AssignInputs::default()
            })
            .collect();
        feats[feat_idx].def.apply(
            feats[feat_idx].pending.level,
            &mut trial,
            WhenCondition::OnFeatureAdd,
            &inputs,
        );
        return solve_assign(feats, feat_idx + 1, 0, &trial, target, attempts);
    }

    // Snapshot fields up-front so the recursive `solve_assign` call can
    // take `&mut feats`. `expr` is Arc-backed, `forced` clone is negligible
    // (Option<Vec<i32>> of ≤18 i32). `mask`/`arg_range` are `Copy`.
    let (arg_count, mask, arg_range, expr, forced) = {
        let assign = &feats[feat_idx].assigns[assign_idx];
        (
            assign.args.len(),
            assign.mask,
            assign.arg_range,
            assign.expr.clone(),
            assign.forced.clone(),
        )
    };

    let mut fallback: Option<Vec<i32>> = None;
    for candidate in enumerate_assign(
        mask,
        arg_range,
        arg_count,
        forced.as_deref(),
        baseline,
        target,
    ) {
        if *attempts == 0 {
            break;
        }
        *attempts -= 1;
        // Zero-vector bypass: always try the no-op candidate, even if the
        // guard would reject it. `feat_def.apply` logs-and-skips on guard
        // failure, so zero args mean "feat contributes nothing" — valid
        // whenever this feat has no diff in target. Guards that forbid
        // zero (e.g. `in(@ARG, 1, 3)`) force non-zero candidates to prove
        // themselves via `passes_guard` on later iterations; if none pass,
        // the solver ultimately returns false and the caller opens the modal.
        let is_zero = candidate.iter().all(|&v| v == 0);
        if !is_zero && !passes_guard(&expr, baseline, &candidate) {
            continue;
        }
        // Remember the first *informative* candidate as fallback for the
        // modal prefill: guard-passing, non-zero, AND whose apply actually
        // changes derived state on `baseline`. Without the effectiveness
        // check, a guard that sums ARGs over the full mask (e.g.
        // `fold(+, @, @ARG) == 2`) would accept args on non-active slots
        // (e.g. Expertise bumps on non-proficient skills, body `if(@==1,…)`
        // no-ops). The modal then sees `is_satisfied=true` from hidden
        // signals and disables every visible checkbox.
        if fallback.is_none()
            && !is_zero
            && candidate_changes_baseline(feats, feat_idx, assign_idx, &candidate, baseline)
        {
            fallback = Some(candidate.clone());
        }
        feats[feat_idx].assigns[assign_idx].args = candidate;
        if solve_assign(feats, feat_idx, assign_idx + 1, baseline, target, attempts) {
            return true;
        }
    }
    // Exhausted — restore the first effective guard-passing candidate so
    // the modal opens pre-filled with a plausible choice. If nothing
    // matched, fall back to zeros (is_satisfied will be false, all
    // active checkboxes stay enabled).
    feats[feat_idx].assigns[assign_idx].args = fallback.unwrap_or_else(|| vec![0; arg_count]);
    false
}

/// Dry-run apply the whole feat (substituting `candidate` for
/// `assigns[assign_idx]`) on `baseline`; return true when derived state
/// actually changes. Used by fallback-prefill selection to reject
/// candidates that satisfy the guard via non-active slots but whose body
/// evaluates to no-op.
fn candidate_changes_baseline(
    feats: &[FeatState<'_>],
    feat_idx: usize,
    assign_idx: usize,
    candidate: &[i32],
    baseline: &Character,
) -> bool {
    let inputs: Vec<AssignInputs> = feats[feat_idx]
        .assigns
        .iter()
        .enumerate()
        .map(|(i, assign)| {
            let args = if i == assign_idx {
                candidate.to_vec()
            } else {
                assign.args.clone()
            };
            AssignInputs {
                args,
                ..AssignInputs::default()
            }
        })
        .collect();
    let mut trial = baseline.clone_lean();
    feats[feat_idx].def.apply(
        feats[feat_idx].pending.level,
        &mut trial,
        WhenCondition::OnFeatureAdd,
        &inputs,
    );
    !baseline.eq_derived(&trial)
}

/// Top-level entry. Returns `true` when the solver found args that make the
/// applied feats' derived state match `target`; `false` otherwise (caller
/// shows modal with whatever partial args remain). Capped by
/// `MAX_TOTAL_ATTEMPTS` to bound worst-case runtime.
pub fn solve_all(feats: &mut [FeatState<'_>], baseline: &Character, target: &Character) -> bool {
    let mut attempts = MAX_TOTAL_ATTEMPTS;
    solve_assign(feats, 0, 0, baseline, target, &mut attempts)
}

#[cfg(test)]
mod tests {
    use strum::VariantArray;
    use wasm_bindgen_test::*;

    use super::*;
    use crate::model::{Ability, FeatureSource, ProficiencyLevel, Skill};

    fn feat_def_single(expr_src: &str) -> FeatureDefinition {
        serde_json::from_value(serde_json::json!({
            "name": "_Test",
            "assign": [{ "expr": expr_src, "when": "OnFeatureAdd" }],
        }))
        .expect("feature def")
    }

    fn pending_l1(name: &str) -> PendingFeature {
        PendingFeature {
            name: name.into(),
            source: FeatureSource::User(0),
            level: 1,
        }
    }

    fn mk_feat_state<'a>(
        def: &'a FeatureDefinition,
        pending: &'a PendingFeature,
        mask: VarSubgroup<AttributeGroup>,
    ) -> FeatState<'a> {
        let assign = def.assign.as_ref().expect("assign");
        let expr = assign[0].expr.clone();
        let arg_count = expr.arg_slot_count(Attribute::arg_index);
        let arg_range = scan_arg_range(&expr).unwrap_or((0, 2));
        FeatState {
            def,
            pending,
            assigns: vec![AssignData {
                expr,
                mask,
                arg_range,
                args: vec![0; arg_count],
                forced: None,
            }],
        }
    }

    #[wasm_bindgen_test]
    fn scan_arg_range_extracts_asi_bounds() {
        let expr: Expr = "with(@ABILITY, guard(fold(and, @, in(@ARG, 0, 2)) and \
             fold(+, @, @ARG) == 2, each(@, if(@ < 20, @ += @ARG))))"
            .parse()
            .unwrap();
        assert_eq!(scan_arg_range(&expr), Some((0, 2)));
    }

    #[wasm_bindgen_test]
    fn solve_single_asi_str_plus_2() {
        let def = feat_def_single(
            "with(@ABILITY, guard(fold(and, @, in(@ARG, 0, 2)) and \
             fold(+, @, @ARG) == 2, each(@, if(@ < 20, @ += @ARG))))",
        );
        let pending = pending_l1("ASI");
        let mut feats = vec![mk_feat_state(
            &def,
            &pending,
            VarSubgroup::from(AttributeGroup::Ability),
        )];

        let mut baseline = Character::default();
        for ability in Ability::VARIANTS {
            baseline.set_ability(*ability, 10);
        }
        let mut target = baseline.clone();
        target.set_ability(Ability::Strength, 12);

        assert!(solve_all(&mut feats, &baseline, &target));
        assert_eq!(feats[0].assigns[0].args[0], 2);
    }

    #[wasm_bindgen_test]
    fn solve_two_asi_plus_spell_sniper_chain() {
        let asi_def = feat_def_single(
            "with(@ABILITY, guard(fold(and, @, in(@ARG, 0, 2)) and \
             fold(+, @, @ARG) == 2, each(@, if(@ < 20, @ += @ARG))))",
        );
        let sniper_def = feat_def_single(
            "with(@ABILITY(INT), guard(fold(and, @, in(@ARG, 0, 1)) and \
             fold(+, @, @ARG) == 1, each(@, if(@ < 20, @ += @ARG))))",
        );

        let p_asi_l4 = pending_l1("ASI-L4");
        let p_sniper = pending_l1("SpellSniper");
        let p_asi_l8 = pending_l1("ASI-L8");
        let mut feats = vec![
            mk_feat_state(
                &asi_def,
                &p_asi_l4,
                VarSubgroup::from(AttributeGroup::Ability),
            ),
            mk_feat_state(
                &sniper_def,
                &p_sniper,
                VarSubgroup::masked(AttributeGroup::Ability, 1 << 3),
            ),
            mk_feat_state(
                &asi_def,
                &p_asi_l8,
                VarSubgroup::from(AttributeGroup::Ability),
            ),
        ];

        let baseline = Character::default(); // INT=8 default
        let mut target = baseline.clone();
        target.set_ability(Ability::Intelligence, 13);

        assert!(solve_all(&mut feats, &baseline, &target));
        assert_eq!(feats[0].assigns[0].args[3], 2, "ASI-L4 INT");
        assert_eq!(feats[1].assigns[0].args[0], 1, "SpellSniper INT");
        assert_eq!(feats[2].assigns[0].args[3], 2, "ASI-L8 INT");
    }

    #[wasm_bindgen_test]
    fn solve_bard_cp_plus_skilled_splits_6_skills() {
        let def = feat_def_single(
            "with(@SKILL._.PROF, guard(fold(and, @, in(@ARG, 0, 1)) and fold(+, @, @ARG) == 3, \
             each(@, if(@ == 0, @ += @ARG))))",
        );
        let mask = VarSubgroup::from(AttributeGroup::SkillProf);
        let p_bard = pending_l1("Bard CP");
        let p_skilled = pending_l1("Skilled");
        let mut feats = vec![
            mk_feat_state(&def, &p_bard, mask),
            mk_feat_state(&def, &p_skilled, mask),
        ];
        let baseline = Character::default();
        let mut target = baseline.clone();
        for skill in [
            Skill::Acrobatics,
            Skill::Athletics,
            Skill::Deception,
            Skill::Investigation,
            Skill::Persuasion,
            Skill::SleightOfHand,
        ] {
            target.skills.set(skill, ProficiencyLevel::Proficient);
        }

        assert!(solve_all(&mut feats, &baseline, &target));
        assert_eq!(feats[0].assigns[0].args.iter().sum::<i32>(), 3);
        assert_eq!(feats[1].assigns[0].args.iter().sum::<i32>(), 3);
    }

    #[wasm_bindgen_test]
    fn solve_unreachable_target_returns_false() {
        let def = feat_def_single("with(@ABILITY(STR), guard(in(@ARG, 0, 1), each(@, @ += @ARG)))");
        let pending = pending_l1("STR-bump");
        let mut feats = vec![mk_feat_state(
            &def,
            &pending,
            VarSubgroup::masked(AttributeGroup::Ability, 0b1),
        )];
        let baseline = Character::default();
        let mut target = baseline.clone();
        target.set_ability(Ability::Dexterity, 10);
        assert!(!solve_all(&mut feats, &baseline, &target));
    }

    #[wasm_bindgen_test]
    fn solve_multi_assign_feat_solves_both() {
        // Simulate a "Dragonscarred"-style feat: ability bump (sum==1) + one
        // skill proficiency (sum==1). Both assigns interactive. Solver must
        // produce args for both before apply.
        let def: FeatureDefinition = serde_json::from_value(serde_json::json!({
            "name": "_Dragonscarred",
            "assign": [
                {
                    "expr": "with(@ABILITY, guard(fold(and, @, in(@ARG, 0, 1)) and \
                             fold(+, @, @ARG) == 1, each(@, if(@ < 20, @ += @ARG))))",
                    "when": "OnFeatureAdd",
                },
                {
                    "expr": "with(@SKILL._.PROF, guard(fold(and, @, in(@ARG, 0, 1)) and \
                             fold(+, @, @ARG) == 1, each(@, if(@ == 0, @ += @ARG))))",
                    "when": "OnFeatureAdd",
                },
            ],
        }))
        .expect("multi-assign def");
        let assigns = def.assign.as_ref().unwrap();
        let ability_expr = assigns[0].expr.clone();
        let skill_expr = assigns[1].expr.clone();
        let ability_arg_count = ability_expr.arg_slot_count(Attribute::arg_index);
        let skill_arg_count = skill_expr.arg_slot_count(Attribute::arg_index);
        let ability_range = scan_arg_range(&ability_expr).unwrap_or((0, 1));
        let skill_range = scan_arg_range(&skill_expr).unwrap_or((0, 1));

        let pending = pending_l1("_Dragonscarred");
        let mut feats = vec![FeatState {
            def: &def,
            pending: &pending,
            assigns: vec![
                AssignData {
                    expr: ability_expr,
                    mask: VarSubgroup::from(AttributeGroup::Ability),
                    arg_range: ability_range,
                    args: vec![0; ability_arg_count],
                    forced: None,
                },
                AssignData {
                    expr: skill_expr,
                    mask: VarSubgroup::from(AttributeGroup::SkillProf),
                    arg_range: skill_range,
                    args: vec![0; skill_arg_count],
                    forced: None,
                },
            ],
        }];

        let mut baseline = Character::default();
        for ability in Ability::VARIANTS {
            baseline.set_ability(*ability, 10);
        }
        let mut target = baseline.clone();
        target.set_ability(Ability::Charisma, 11); // +1 CHA
        target
            .skills
            .set(Skill::Intimidation, ProficiencyLevel::Proficient); // +1 Intimidation

        assert!(solve_all(&mut feats, &baseline, &target));
        // CHA = index 5 in ability order.
        assert_eq!(feats[0].assigns[0].args[5], 1, "CHA bump");
        assert_eq!(feats[0].assigns[0].args.iter().sum::<i32>(), 1);
        // Intimidation picked in skill assign.
        assert_eq!(feats[0].assigns[1].args.iter().sum::<i32>(), 1);
    }

    /// Reproduce the core Хельма (Copy) scenario: stored Expertise forced
    /// before CP in the `feats` array (because the solver iterates in
    /// `all_pending` pipeline order). CP is unsolved — solver picks 4 skill
    /// PROFs. Expertise forced args target Acro + SleightOfHand; after CP
    /// grants Acro PROF, Expertise's `if(@ == 1, @ += @ARG)` bumps Acro to
    /// Expertise. SleightOfHand already PROF in baseline (simulating
    /// Background Abilities).
    #[wasm_bindgen_test]
    fn solve_cp_before_expertise_forced_pipeline() {
        let cp_def: FeatureDefinition = serde_json::from_value(serde_json::json!({
            "name": "Class Proficiencies (Rogue)",
            "assign": [{
                "expr": "with(@SKILL._.PROF(ACRO, ATHL, DECE, INSI, INTI, INVE, PERC, PERF, PERS, SLEI, STEA), \
                         guard(fold(and, @, in(@ARG, 0, 1)) and fold(+, @, @ARG) == 4, \
                         each(@, if(@ == 0, @ += @ARG))))",
                "when": "OnFeatureAdd",
            }],
        })).expect("cp def");
        let expertise_def: FeatureDefinition = serde_json::from_value(serde_json::json!({
            "name": "Expertise",
            "assign": [{
                "expr": "with(@SKILL._.PROF, guard(fold(and, @, in(@ARG, 0, 1)) and \
                         fold(+, @, @ARG) == 2, each(@, if(@ == 1, @ += @ARG))))",
                "when": "OnFeatureAdd",
            }],
        }))
        .expect("expertise def");

        // CP mask = bits for {Acro, Athl, Dec, Insi, Inti, Inv, Perc, Perf, Pers,
        // Sleight, Stealth}.
        let cp_mask = VarSubgroup::masked(
            AttributeGroup::SkillProf,
            (1u32 << 0)   // Acrobatics
                | (1 << 3)   // Athletics
                | (1 << 4)   // Deception
                | (1 << 6)   // Insight
                | (1 << 7)   // Intimidation
                | (1 << 8)   // Investigation
                | (1 << 11)  // Perception
                | (1 << 12)  // Performance
                | (1 << 13)  // Persuasion
                | (1 << 15)  // SleightOfHand
                | (1 << 16), // Stealth
        );

        let p_cp = pending_l1("Class Proficiencies (Rogue)");
        let p_exp = pending_l1("Expertise");

        // Expertise forced args: bump Acro (pos 0) and Sleight (pos 15) in
        // the full 18-skill SkillProf group.
        let mut expertise_forced = vec![0i32; 18];
        expertise_forced[0] = 1;
        expertise_forced[15] = 1;

        let mut feats = vec![
            mk_feat_state(&cp_def, &p_cp, cp_mask),
            FeatState {
                def: &expertise_def,
                pending: &p_exp,
                assigns: vec![AssignData {
                    expr: expertise_def.assign.as_ref().unwrap()[0].expr.clone(),
                    mask: VarSubgroup::from(AttributeGroup::SkillProf),
                    arg_range: (0, 1),
                    args: vec![0; 18],
                    forced: Some(expertise_forced),
                }],
            },
        ];

        // Baseline: SleightOfHand already PROF (simulating Criminal
        // background applied earlier). Everything else zero.
        let mut baseline = Character::default();
        baseline
            .skills
            .set(Skill::SleightOfHand, ProficiencyLevel::Proficient);
        baseline
            .skills
            .set(Skill::Stealth, ProficiencyLevel::Proficient);

        // Target: CP picks (Acrobatics, Athletics, Deception, Investigation);
        // Expertise bumps Acro + Sleight to Expertise; Stealth stays at PROF.
        let mut target = baseline.clone();
        target
            .skills
            .set(Skill::Acrobatics, ProficiencyLevel::Expertise);
        target
            .skills
            .set(Skill::Athletics, ProficiencyLevel::Proficient);
        target
            .skills
            .set(Skill::Deception, ProficiencyLevel::Proficient);
        target
            .skills
            .set(Skill::Investigation, ProficiencyLevel::Proficient);
        target
            .skills
            .set(Skill::SleightOfHand, ProficiencyLevel::Expertise);

        assert!(
            solve_all(&mut feats, &baseline, &target),
            "solver must find a solution when stored Expertise follows unsolved CP in pipeline"
        );

        // CP picked ACRO, ATHL, DECE, INVE (4 slots out of 11 in mask).
        // Args are indexed by mask's iter_no, so: pos 0 = ACRO, 1 = ATHL,
        // 2 = DECE, 5 = INVE in the 11-member CP mask.
        let cp_args = &feats[0].assigns[0].args;
        assert_eq!(cp_args[0], 1, "CP picks Acrobatics");
        assert_eq!(cp_args[1], 1, "CP picks Athletics");
        assert_eq!(cp_args[2], 1, "CP picks Deception");
        assert_eq!(cp_args[5], 1, "CP picks Investigation");
        assert_eq!(cp_args.iter().sum::<i32>(), 4);

        // Expertise kept its forced args (solver did not mutate a forced assign).
        assert_eq!(feats[1].assigns[0].args[0], 1, "Expertise Acrobatics");
        assert_eq!(feats[1].assigns[0].args[15], 1, "Expertise SleightOfHand");
    }

    /// Reproduce the exact Хельма (Copy) feature set: Criminal BA forced,
    /// CP Rogue unsolved, Expertise forced, ASI unsolved. Target abilities
    /// DEX=18 (requires +10, unreachable with 1 ASI+Criminal = max +3) and
    /// skills Acro=Expertise, Sleight=Expertise, Athl/Dec/Inv/Stealth=PROF.
    /// The character.json also has History/Nature/Religion=PROF which no
    /// feature can grant — so solver returns false by design.
    ///
    /// The test asserts FALLBACK prefills are sensible on that failure:
    /// - CP args have 4 ones in the Rogue mask (Acro, Athl, Dec, Inv).
    /// - ASI args have a valid +2 split passing its guard.
    #[wasm_bindgen_test]
    fn helma_copy_unreachable_target_still_gives_fallback_prefill() {
        let cp_def: FeatureDefinition = serde_json::from_value(serde_json::json!({
            "name": "Class Proficiencies (Rogue)",
            "assign": [{
                "expr": "with(@SKILL._.PROF(ACRO, ATHL, DECE, INSI, INTI, INVE, PERC, PERF, PERS, SLEI, STEA), \
                         guard(fold(and, @, in(@ARG, 0, 1)) and fold(+, @, @ARG) == 4, \
                         each(@, if(@ == 0, @ += @ARG))))",
                "when": "OnFeatureAdd",
            }],
        })).unwrap();
        let criminal_ba_def: FeatureDefinition = serde_json::from_value(serde_json::json!({
            "name": "Background Abilities (Criminal)",
            "assign": [{
                "expr": "with(@ABILITY(DEX, CON, INT), \
                         guard(fold(and, @, in(@ARG, 0, 2) and @ + @ARG <= 20) and \
                         fold(+, @, @ARG) == 3, each(@, if(@ < 20, @ += @ARG)))); \
                         SKILL.SLEI.PROF = 1; SKILL.STEA.PROF = 1",
                "when": "OnFeatureAdd",
            }],
        }))
        .unwrap();
        let expertise_def: FeatureDefinition = serde_json::from_value(serde_json::json!({
            "name": "Expertise",
            "assign": [{
                "expr": "with(@SKILL._.PROF, guard(fold(and, @, in(@ARG, 0, 1)) and \
                         fold(+, @, @ARG) == 2, each(@, if(@ == 1, @ += @ARG))))",
                "when": "OnFeatureAdd",
            }],
        }))
        .unwrap();
        let asi_def: FeatureDefinition = serde_json::from_value(serde_json::json!({
            "name": "Ability Score Improvement",
            "assign": [{
                "expr": "with(@ABILITY, guard(fold(and, @, in(@ARG, 0, 2) and @ + @ARG <= 20) \
                         and fold(+, @, @ARG) == 2, each(@, if(@ < 20, @ += @ARG))))",
                "when": "OnFeatureAdd",
            }],
        }))
        .unwrap();

        let cp_mask = VarSubgroup::masked(
            AttributeGroup::SkillProf,
            (1u32 << 0)  // Acrobatics
                | (1 << 3)   // Athletics
                | (1 << 4)   // Deception
                | (1 << 6)   // Insight
                | (1 << 7)   // Intimidation
                | (1 << 8)   // Investigation
                | (1 << 11)  // Perception
                | (1 << 12)  // Performance
                | (1 << 13)  // Persuasion
                | (1 << 15)  // SleightOfHand
                | (1 << 16), // Stealth
        );

        let p_criminal_ba = pending_l1("Background Abilities (Criminal)");
        let p_cp = pending_l1("Class Proficiencies (Rogue)");
        let p_exp = pending_l1("Expertise");
        let p_asi = PendingFeature {
            name: "Ability Score Improvement".into(),
            source: FeatureSource::Class("Rogue".into(), 4),
            level: 4,
        };

        let expertise_expr = expertise_def.assign.as_ref().unwrap()[0].expr.clone();
        let expertise_forced = {
            let mut args = vec![0i32; 18];
            args[0] = 1;
            args[15] = 1;
            args
        };

        let criminal_ba_expr = criminal_ba_def.assign.as_ref().unwrap()[0].expr.clone();
        let criminal_ba_arg_count = criminal_ba_expr.arg_slot_count(Attribute::arg_index);

        let mut feats = vec![
            FeatState {
                def: &criminal_ba_def,
                pending: &p_criminal_ba,
                assigns: vec![AssignData {
                    expr: criminal_ba_expr,
                    mask: VarSubgroup::masked(
                        AttributeGroup::Ability,
                        (1 << 1) | (1 << 2) | (1 << 3), // DEX, CON, INT
                    ),
                    arg_range: (0, 2),
                    args: vec![0; criminal_ba_arg_count],
                    forced: Some(vec![1, 1, 1]),
                }],
            },
            mk_feat_state(&cp_def, &p_cp, cp_mask),
            FeatState {
                def: &expertise_def,
                pending: &p_exp,
                assigns: vec![AssignData {
                    expr: expertise_expr,
                    mask: VarSubgroup::from(AttributeGroup::SkillProf),
                    arg_range: (0, 1),
                    args: vec![0; 18],
                    forced: Some(expertise_forced),
                }],
            },
            mk_feat_state(&asi_def, &p_asi, VarSubgroup::from(AttributeGroup::Ability)),
        ];

        // Baseline: identity only — all abilities 8, all skills 0.
        let baseline = Character::default();

        // Target matches Хельма (Copy): unreachable by this feat set.
        let mut target = baseline.clone();
        target.set_ability(Ability::Strength, 12);
        target.set_ability(Ability::Dexterity, 18);
        target.set_ability(Ability::Constitution, 15);
        target.set_ability(Ability::Intelligence, 14);
        target.set_ability(Ability::Wisdom, 10);
        target.set_ability(Ability::Charisma, 8);
        target
            .skills
            .set(Skill::Acrobatics, ProficiencyLevel::Expertise);
        target
            .skills
            .set(Skill::Athletics, ProficiencyLevel::Proficient);
        target
            .skills
            .set(Skill::Deception, ProficiencyLevel::Proficient);
        target
            .skills
            .set(Skill::History, ProficiencyLevel::Proficient);
        target
            .skills
            .set(Skill::Investigation, ProficiencyLevel::Proficient);
        target
            .skills
            .set(Skill::Nature, ProficiencyLevel::Proficient);
        target
            .skills
            .set(Skill::Religion, ProficiencyLevel::Proficient);
        target
            .skills
            .set(Skill::SleightOfHand, ProficiencyLevel::Expertise);
        target
            .skills
            .set(Skill::Stealth, ProficiencyLevel::Proficient);

        let solved = solve_all(&mut feats, &baseline, &target);
        // Unreachable by feat set — History/Nature/Religion off-mask, and
        // DEX=18 needs +10 while Criminal(+1) + ASI(+2) max out at +3.
        assert!(
            !solved,
            "Хельма (Copy) target is unreachable — solver must fail"
        );

        // Still, fallback should pre-fill CP with 4 bumps inside its mask,
        // not all zeros (those are the target-reachable diff slots).
        let cp_args = &feats[1].assigns[0].args;
        let cp_sum: i32 = cp_args.iter().sum();
        assert_eq!(
            cp_sum, 4,
            "CP fallback must pick exactly 4 skills (guard sum==4 passing candidate); got {cp_args:?}"
        );
        // Acrobatics at position 0 in CP mask — target-diff says we need it.
        assert_eq!(cp_args[0], 1, "CP fallback picks Acrobatics");

        // ASI fallback: some valid +2 split (first guard-passing candidate).
        let asi_args = &feats[3].assigns[0].args;
        let asi_sum: i32 = asi_args.iter().sum();
        assert_eq!(asi_sum, 2, "ASI fallback must be a valid sum==2 candidate");
    }
}
