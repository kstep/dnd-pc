use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use leptos::prelude::*;
use leptos_fluent::move_tr;

use crate::{
    components::icon::Icon,
    expr::{
        self, BLOCK_ERROR, BLOCK_NOOP, Block, Context, CursorStack, DicePool, ExprAnalysis,
        GroupCursor,
    },
    model::{AssignInputs, Attribute, AttributeGroup, CharacterCore, Expr, Op, StaticAttrSource},
};

// --- ArgContext: resolves Arg(n) from signals, delegates rest to CharacterCore
// ---

struct ArgContext<'a> {
    character: &'a CharacterCore,
    args: &'a [Signal<i32>],
}

impl AsRef<CharacterCore> for ArgContext<'_> {
    fn as_ref(&self) -> &CharacterCore {
        self.character
    }
}

impl Context<Attribute, i32> for ArgContext<'_> {
    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        match var {
            Attribute::Arg(n) => Ok(self.args.get(n as usize).map_or(0, |signal| signal.get())),
            other => self.character.resolve(other),
        }
    }

    fn assign(&mut self, var: Attribute, _: i32) -> Result<(), expr::Error> {
        Err(expr::Error::read_only_var(var))
    }
}

struct ProbeContext<'a> {
    character: &'a CharacterCore,
    args: Vec<i32>,
}

impl AsRef<CharacterCore> for ProbeContext<'_> {
    fn as_ref(&self) -> &CharacterCore {
        self.character
    }
}

impl Context<Attribute, i32> for ProbeContext<'_> {
    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        match var {
            Attribute::Arg(n) => Ok(self.args.get(n as usize).copied().unwrap_or(0)),
            other => self.character.resolve(other),
        }
    }

    fn assign(&mut self, var: Attribute, _: i32) -> Result<(), expr::Error> {
        Err(expr::Error::read_only_var(var))
    }
}

struct PartialEvalCtx<'a> {
    args: &'a [RwSignal<i32>],
}

// No Character: group iteration yields nothing.
impl expr::ResolveGroup<AttributeGroup> for PartialEvalCtx<'_> {
    fn resolve_group<'a>(
        &'a self,
        _grp: &AttributeGroup,
    ) -> Box<dyn Iterator<Item = Vec<Attribute>> + 'a> {
        Box::new(std::iter::empty())
    }
}

impl Context<Attribute, i32> for PartialEvalCtx<'_> {
    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        match var {
            Attribute::Arg(n) => Ok(self.args.get(n as usize).map_or(0, |s| s.get())),
            other => Err(expr::Error::unsupported_var(other)),
        }
    }

    fn assign(&mut self, var: Attribute, _: i32) -> Result<(), expr::Error> {
        Err(expr::Error::read_only_var(var))
    }
}

// --- FormBuilder: view-building stack mirroring Formatter ---

struct FormBuilder(Vec<AnyView>);

impl FormBuilder {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn push_text(&mut self, s: impl std::fmt::Display) {
        let text = s.to_string();
        self.0.push(text.into_any());
    }

    fn push_view(&mut self, v: AnyView) {
        self.0.push(v);
    }

    fn pop(&mut self) -> Result<AnyView, expr::Error> {
        self.0.pop().ok_or(expr::Error::StackUnderflow)
    }

    fn pop2(&mut self) -> Result<(AnyView, AnyView), expr::Error> {
        let b = self.pop()?;
        let a = self.pop()?;
        Ok((a, b))
    }

    fn binary_op(&mut self, sym: &'static str) -> Result<(), expr::Error> {
        let (a, b) = self.pop2()?;
        self.0.push(view! { <>{a}" "{sym}" "{b}</> }.into_any());
        Ok(())
    }

    fn binary_func(&mut self, name: &'static str) -> Result<(), expr::Error> {
        let (a, b) = self.pop2()?;
        self.0
            .push(view! { <>{name}"("{a}", "{b}")"</> }.into_any());
        Ok(())
    }

    fn exec_op(&mut self, op: Op, i18n: &leptos_fluent::I18n) -> Result<(), expr::Error> {
        match op {
            Op::PushConst(n) => self.push_text(n),
            Op::PushVar(var) => {
                let i18n = *i18n;
                self.push_view((move || var.display_name(i18n)).into_any());
            }
            Op::BinOp(bin_op) if bin_op.is_func() => self.binary_func(bin_op.symbol())?,
            Op::BinOp(bin_op) => self.binary_op(bin_op.symbol())?,
            Op::Roll => {
                let (count, sides) = self.pop2()?;
                self.0.push(view! { <>{count}"d"{sides}</> }.into_any());
            }
            Op::Sum => {} // follows Roll, already on stack
            Op::Explode => {
                let roll = self.pop()?;
                self.0.push(view! { <>{roll}"!"</> }.into_any());
            }
            Op::KeepMax(n) => {
                let roll = self.pop()?;
                self.0.push(view! { <>{roll}"kh"{n}</> }.into_any());
            }
            Op::KeepMin(n) => {
                let roll = self.pop()?;
                self.0.push(view! { <>{roll}"kl"{n}</> }.into_any());
            }
            Op::DropMax(n) => {
                let roll = self.pop()?;
                self.0.push(view! { <>{roll}"dh"{n}</> }.into_any());
            }
            Op::DropMin(n) => {
                let roll = self.pop()?;
                self.0.push(view! { <>{roll}"dl"{n}</> }.into_any());
            }
            Op::AvgHp => {
                let a = self.pop()?;
                self.0.push(view! { <>"avg_hp("{a}")"</> }.into_any());
            }
            Op::Not => {
                let a = self.pop()?;
                self.0.push(view! { <>"not "{a}</> }.into_any());
            }
            Op::Cmp(cmp) => self.binary_op(cmp.symbol())?,
            Op::AssignVar(var) => {
                let val = self.pop()?;
                let i18n = *i18n;
                let var_s = move || var.display_name(i18n);
                self.0.push(view! { <>{var_s}" = "{val}</> }.into_any());
            }
            Op::In => {
                let c = self.pop()?;
                let b = self.pop()?;
                let a = self.pop()?;
                self.0
                    .push(view! { <>"in("{a}", "{b}", "{c}")"</> }.into_any());
            }
            Op::Eval(_)
            | Op::EvalIf(_, _)
            | Op::Each(_)
            | Op::Next
            | Op::PushGroup(_, _)
            | Op::AssignGroup(_, _)
            | Op::Tier(_) => {} // intercepted by form_block
        }
        Ok(())
    }

    fn finish(self) -> Result<AnyView, expr::Error> {
        if self.0.is_empty() {
            return Err(expr::Error::EmptyExpression);
        }
        if self.0.len() == 1 {
            return Ok(self.0.into_iter().next().unwrap());
        }
        // Each statement in its own line
        Ok(self
            .0
            .into_iter()
            .map(|element| view! { <div class="expr-formula-line">{element}</div> }.into_any())
            .collect_view()
            .into_any())
    }
}

// --- form_block: recursive block walker producing views ---

/// Context for form building: tracks which ARGs are active (from analysis),
/// which have been seen (first occurrence = input, later = read-only ref),
/// and the arg signals.
struct FormCtx<'a> {
    args: &'a [RwSignal<i32>],
    seen: BTreeSet<u8>,
    active_args: BTreeSet<u8>,
    boolean_args: BTreeSet<u8>,
    i18n: leptos_fluent::I18n,
    iter_stack: CursorStack<Attribute>,
    is_satisfied: Memo<bool>,
}

impl<'a> FormCtx<'a> {
    fn new(
        args: &'a [RwSignal<i32>],
        active_args: BTreeSet<u8>,
        boolean_args: BTreeSet<u8>,
        i18n: leptos_fluent::I18n,
        is_satisfied: Memo<bool>,
    ) -> Self {
        Self {
            args,
            seen: BTreeSet::new(),
            active_args,
            boolean_args,
            i18n,
            iter_stack: CursorStack::new(),
            is_satisfied,
        }
    }

    fn is_active(&self, n: u8) -> bool {
        self.active_args.contains(&n)
    }

    fn is_boolean(&self, n: u8) -> bool {
        self.boolean_args.contains(&n)
    }
}

fn form_block(
    expr: &Expr,
    block: expr::BlockIndex,
    ctx: &mut FormCtx<'_>,
    condition: bool,
) -> Result<AnyView, expr::Error> {
    let mut fb = FormBuilder::new();
    render_block_inline(&mut fb, expr, block, ctx, condition)?;
    fb.finish()
}

fn render_block_inline(
    fb: &mut FormBuilder,
    expr: &Expr,
    block: expr::BlockIndex,
    ctx: &mut FormCtx<'_>,
    condition: bool,
) -> Result<(), expr::Error> {
    let block_ops = &**expr.block(block);
    let mut start = 0;
    for (i, op) in block_ops.iter().enumerate() {
        if matches!(op, Op::AssignVar(_) | Op::AssignGroup(_, _)) {
            render_statement(fb, expr, block, start..(i + 1), ctx, condition)?;
            start = i + 1;
        }
    }
    if start < block_ops.len() {
        render_statement(fb, expr, block, start..block_ops.len(), ctx, condition)?;
    }
    Ok(())
}

/// Render a single statement: detect compound assignment (`X += rhs`) and
/// render as `{label} {op}= {rhs_value}`, or fall back to `form_block_ops`.
/// The RHS is collapsed to its live computed value via `Expr::eval_ops` so
/// nested folds/sums show their result instead of per-iter cells.
fn render_statement(
    fb: &mut FormBuilder,
    expr: &Expr,
    block: expr::BlockIndex,
    stmt_range: std::ops::Range<usize>,
    ctx: &mut FormCtx<'_>,
    condition: bool,
) -> Result<(), expr::Error> {
    let stmt = &(**expr.block(block))[stmt_range.clone()];
    let Some(compound) = Block::detect_compound(stmt) else {
        return form_block_ops(fb, expr, stmt, ctx, condition);
    };
    if compound.prefix_end > 0 {
        form_block_ops(fb, expr, &stmt[..compound.prefix_end], ctx, condition)?;
    }
    let i18n = ctx.i18n;
    let var_view: AnyView = match stmt.last() {
        Some(&Op::AssignVar(var)) => (move || var.display_name(i18n)).into_any(),
        Some(&Op::AssignGroup(_grp, col)) => {
            let var = ctx
                .iter_stack
                .top()
                .ok()
                .and_then(|cursor| cursor.col(col).copied())
                .expect("cursor live with valid column");
            (move || var.display_name(i18n)).into_any()
        }
        _ => unreachable!(),
    };
    let sym = compound.sym;
    let rhs_slice = &stmt[compound.rhs_start..compound.rhs_end];
    if rhs_has_reductive_loop(expr, rhs_slice) {
        // Collapse the whole RHS to its live value — folds/sums inside
        // would otherwise pile per-iter `@ARG` reads onto the formula.
        let rhs_start = stmt_range.start + compound.rhs_start;
        let rhs_end = stmt_range.start + compound.rhs_end;
        let expr_owned = expr.clone();
        let signals: Vec<RwSignal<i32>> = ctx.args.to_vec();
        let compute = move || {
            let rhs_ops = &(**expr_owned.block(block))[rhs_start..rhs_end];
            let eval_ctx = PartialEvalCtx { args: &signals };
            expr_owned.eval_ops(rhs_ops, &eval_ctx).unwrap_or(0)
        };
        fb.push_view(view! { <>{var_view}" "{sym}"= "{compute}</> }.into_any());
    } else {
        // Render the RHS as form elements so first-time `@ARG` references
        // surface as interactive inputs (checkboxes / number fields).
        form_block_ops(fb, expr, rhs_slice, ctx, condition)?;
        let rhs = fb.pop()?;
        fb.push_view(view! { <>{var_view}" "{sym}"= "{rhs}</> }.into_any());
    }
    Ok(())
}

fn rhs_has_reductive_loop(expr: &Expr, ops: &[Op]) -> bool {
    ops.iter().enumerate().any(|(i, op)| {
        let Op::Each(_) = op else {
            return false;
        };
        let Some(Op::EvalIf(body_idx, _)) = ops.get(i + 1) else {
            return false;
        };
        !expr.block_assigns_to(*body_idx, &|_| true)
    })
}

fn arg_checkbox(signal: RwSignal<i32>, is_satisfied: Memo<bool>) -> AnyView {
    view! {
        <input
            type="checkbox"
            class="expr-form-input"
            prop:checked=move || signal.get() != 0
            prop:disabled=move || is_satisfied.get() && signal.get() == 0
            on:change=move |ev| {
                signal.set(event_target_checked(&ev) as i32);
            }
        />
    }
    .into_any()
}

fn arg_number(signal: RwSignal<i32>, is_satisfied: Memo<bool>) -> AnyView {
    view! {
        <input
            type="number"
            class="expr-form-input"
            prop:value=move || signal.get()
            prop:disabled=move || is_satisfied.get() && signal.get() == 0
            on:input=move |ev| {
                let value = event_target_value(&ev).parse::<i32>().unwrap_or(0);
                signal.set(value);
            }
        />
    }
    .into_any()
}

fn arg_ref(signal: RwSignal<i32>) -> AnyView {
    view! { <span class="expr-form-ref">{move || signal.get()}</span> }.into_any()
}

fn form_block_ops(
    fb: &mut FormBuilder,
    expr: &Expr,
    ops: &[Op],
    ctx: &mut FormCtx<'_>,
    condition: bool,
) -> Result<(), expr::Error> {
    let mut i = 0;
    while i < ops.len() {
        let op = ops[i];
        match op {
            // Loop: Each(grp) + EvalIf(body, NOOP) → unroll
            Op::Each(grp) if i + 1 < ops.len() => {
                if let Op::EvalIf(body_idx, BLOCK_NOOP) = ops[i + 1] {
                    form_block_loop(fb, expr, grp, body_idx, ctx, condition)?;
                    i += 2;
                    continue;
                }
            }
            Op::PushVar(Attribute::Arg(n)) => {
                push_arg_input(fb, ctx, n, condition);
            }
            Op::PushGroup(_grp, col) => {
                if let Some(var) = ctx
                    .iter_stack
                    .top()
                    .ok()
                    .and_then(|cursor| cursor.col(col).copied())
                {
                    if let Attribute::Arg(n) = var {
                        push_arg_input(fb, ctx, n, condition);
                    } else {
                        let i18n = ctx.i18n;
                        fb.push_view((move || var.display_name(i18n)).into_any());
                    }
                }
            }
            Op::AssignGroup(_grp, col) => {
                if let Some(var) = ctx
                    .iter_stack
                    .top()
                    .ok()
                    .and_then(|cursor| cursor.col(col).copied())
                {
                    let val = fb.pop()?;
                    let i18n = ctx.i18n;
                    let var_s = move || var.display_name(i18n);
                    fb.push_view(view! { <>{var_s}" = "{val}</> }.into_any());
                }
            }
            Op::Next => {} // handled by loop unroll
            Op::Eval(idx) => {
                let sub = form_block(expr, idx, ctx, true)?;
                fb.push_view(sub);
            }
            Op::EvalIf(then_idx, else_idx) => {
                let cond = fb.pop()?;
                let is_active_arg =
                    |var: &Attribute| matches!(var, Attribute::Arg(n) if ctx.is_active(*n));
                let then_has_args = expr.block_has_var(then_idx, &is_active_arg);
                let else_has_args =
                    else_idx != BLOCK_NOOP && expr.block_has_var(else_idx, &is_active_arg);

                if !then_has_args && !else_has_args {
                    i += 1;
                    continue;
                }

                if else_idx == BLOCK_ERROR {
                    // Guard body: inline statements into the current
                    // FormBuilder so each one goes through compound-assign
                    // detection (same wiring as `form_block`, just without
                    // the outer finish/wrap).
                    render_block_inline(fb, expr, then_idx, ctx, condition)?;
                } else if else_idx == BLOCK_NOOP {
                    let then_view = form_block(expr, then_idx, ctx, false)?;
                    fb.push_view(then_view);
                } else {
                    let then_view = form_block(expr, then_idx, ctx, false)?;
                    if else_has_args {
                        let else_view = form_block(expr, else_idx, ctx, false)?;
                        fb.push_view(
                            view! { <>"if("{cond}", "{then_view}", "{else_view}")"</> }.into_any(),
                        );
                    } else {
                        fb.push_view(view! { <>"if("{cond}", "{then_view}")"</> }.into_any());
                    }
                }
            }
            op => fb.exec_op(op, &ctx.i18n)?,
        }
        i += 1;
    }
    Ok(())
}

/// Unroll a loop: render the body block once per group member.
/// Skips iterations where the body references `@ARG` (companion ARG group)
/// for an iter_no not in `ctx.active_args` — their assignments would be
/// `SKILL[i].PROF += ARG.i = 0` which pollutes the view with zero-effect
/// rows the user can't interact with.
fn form_block_loop(
    fb: &mut FormBuilder,
    expr: &Expr,
    subgrp: expr::VarSubgroup<AttributeGroup>,
    body_idx: expr::BlockIndex,
    ctx: &mut FormCtx<'_>,
    condition: bool,
) -> Result<(), expr::Error> {
    // Reductive loops (fold/sum) — render a single span with the live value
    // instead of per-iter `@ARG` reads. Keeps the form-builder stack balanced
    // and lets the surrounding formula read e.g. `Tool slots += 3 - 2`.
    if !expr.block_assigns_to(body_idx, &|_| true) {
        fb.push_view(render_fold_value(expr, subgrp, body_idx, ctx));
        return Ok(());
    }
    let body_len = expr.block(body_idx).len();
    // Strip trailing Next + EvalIf (loop control ops)
    let content_end = body_len.saturating_sub(2);
    let body_uses_arg_group = expr.block_has_var(body_idx, &|var| matches!(var, Attribute::Arg(_)));
    let cursor = GroupCursor::build(&StaticAttrSource, &subgrp);
    if !cursor.is_live() {
        return Ok(());
    }
    ctx.iter_stack.push(cursor);
    loop {
        let row_no = ctx
            .iter_stack
            .top()
            .ok()
            .map(|cursor| cursor.row_no())
            .unwrap_or(0);
        if !body_uses_arg_group || ctx.is_active(row_no as u8) {
            render_statement(fb, expr, body_idx, 0..content_end, ctx, condition)?;
        }
        let more = ctx
            .iter_stack
            .top_mut()
            .map(|cursor| cursor.advance())
            .unwrap_or(false);
        if !more {
            break;
        }
    }
    let _ = ctx.iter_stack.pop();
    Ok(())
}

/// Live value of a reductive loop (fold/sum). Re-uses `Expr::eval_ops` with
/// the loop's driver ops so any binop/shape supported by the evaluator
/// works out of the box.
fn render_fold_value(
    expr: &Expr,
    subgrp: expr::VarSubgroup<AttributeGroup>,
    body_idx: expr::BlockIndex,
    ctx: &FormCtx<'_>,
) -> AnyView {
    let ops = vec![Op::Each(subgrp), Op::EvalIf(body_idx, BLOCK_NOOP)];
    let expr_owned = expr.clone();
    let signals: Vec<RwSignal<i32>> = ctx.args.to_vec();
    let compute = move || {
        let eval_ctx = PartialEvalCtx { args: &signals };
        expr_owned.eval_ops(&ops, &eval_ctx).unwrap_or(0)
    };
    view! { <span class="expr-form-ref">{compute}</span> }.into_any()
}

/// Push an ARG input (checkbox, number, or ref) for the given ARG index.
/// `ctx.args` is pre-sized by `arg_slot_count` during component build, so
/// indexing is always in bounds. `debug_assert!` catches desync between the
/// sizing scan and the runtime interpreter (both must use the same `arg_index`
/// extractor) during development.
fn push_arg_input(fb: &mut FormBuilder, ctx: &mut FormCtx<'_>, n: u8, condition: bool) {
    debug_assert!(
        (n as usize) < ctx.args.len(),
        "ARG.{n} out of bounds (len={}): arg_slot_count and runtime disagree",
        ctx.args.len()
    );
    let signal = ctx.args[n as usize];
    fb.push_view(if !condition && ctx.is_active(n) && ctx.seen.insert(n) {
        if ctx.is_boolean(n) {
            arg_checkbox(signal, ctx.is_satisfied)
        } else {
            arg_number(signal, ctx.is_satisfied)
        }
    } else {
        arg_ref(signal)
    });
}

// --- ExprArgsInput ---

/// Dice values for one die type: sides and per-die signals.
/// A value of 0 means the input is not yet filled (dice are always ≥ 1).
pub type DiceGroupSignals = BTreeMap<u32, Vec<RwSignal<u32>>>;

/// The rendered parts of an expression input: arg signals, dice signals,
/// and validation memo. Returned by `ExprArgsInput` via the `on_ready`
/// callback so the parent can wire up a shared submit button.
pub struct ExprArgsInputParts {
    pub arg_signals: Vec<RwSignal<i32>>,
    pub dice_signals: DiceGroupSignals,
    pub is_valid: Memo<bool>,
}

impl ExprArgsInputParts {
    /// Read all dice input values and return a `DicePool`.
    pub fn collect_dice(&self) -> DicePool {
        collect_dice_pool(&self.dice_signals)
    }
}

/// Read dice signal values into a `DicePool`. Used by ArgsModal and effects.
pub fn collect_dice_pool(groups: &DiceGroupSignals) -> DicePool {
    let map: BTreeMap<u32, Vec<u32>> = groups
        .iter()
        .map(|(&sides, signals)| {
            let values: Vec<u32> = signals
                .iter()
                .map(|signal| signal.get_untracked())
                .filter(|&value| value > 0)
                .collect();
            (sides, values)
        })
        .collect();
    map.into()
}

/// Build dice input groups view from roll requirements.
/// Returns the signal map and the rendered view. `prefill` supplies initial
/// values per side (extra requested rolls fall back to 0).
pub fn build_dice_groups(
    dice_rolls: &BTreeMap<u32, u32>,
    prefill: &DicePool,
) -> (DiceGroupSignals, AnyView) {
    let groups: DiceGroupSignals = dice_rolls
        .iter()
        .map(|(&sides, &count)| {
            let preset = prefill.get(sides);
            let signals: Vec<_> = (0..count)
                .map(|i| {
                    let init = preset.get(i as usize).copied().unwrap_or(0);
                    RwSignal::new(init)
                })
                .collect();
            (sides, signals)
        })
        .collect();

    let groups_for_roll = groups.clone();

    let mut first = true;
    let groups_view = groups
        .iter()
        .map(|(&sides, signals)| {
            let input_views = signals
                .iter()
                .map(|&signal| {
                    let is_first = first;
                    first = false;
                    view! {
                        <input
                            type="number"
                            min=1
                            max=sides
                            required
                            autofocus=is_first
                            class="dice-pool-value"
                            prop:value=move || {
                                let value = signal.get();
                                if value == 0 { String::new() } else { value.to_string() }
                            }
                            on:input=move |ev| {
                                let value = event_target_value(&ev).parse::<u32>().unwrap_or(0);
                                signal.set(value);
                            }
                        />
                    }
                })
                .collect_view();
            view! {
                <div class="dice-pool-group">
                    <span class="dice-pool-label">"d" {sides}</span>
                    <div class="dice-pool-inputs">{input_views}</div>
                </div>
            }
        })
        .collect_view();

    let view = view! {
        <button
            type="button"
            class="btn-icon dice-pool-roll-all"
            title=move_tr!("roll-all-dice")
            on:click=move |_| {
                for (&sides, signals) in &groups_for_roll {
                    for signal in signals {
                        let value = getrandom::u32().unwrap_or(0) % sides + 1;
                        signal.set(value);
                    }
                }
            }
        >
            <Icon name="dices" />
        </button>
        {groups_view}
    }
    .into_any();

    (groups, view)
}

/// Renders the interactive formula with number inputs for `ARG.n` variables
/// and dice input groups for any dice rolls in the expression.
/// No submit button — the parent is responsible for submission. Calls
/// `on_ready` synchronously during build with the signals and validation memo.
#[component]
pub fn ExprArgsInput(
    expr: Expr,
    /// Snapshot of the character as seen by this feature during cascade
    /// analysis — the character with all upstream pending features already
    /// applied. Non-cascade call sites can pass a derived signal over the
    /// live `Store<Character>`.
    character: Signal<Arc<CharacterCore>>,
    #[prop(optional)] prefill: AssignInputs,
    on_ready: impl FnOnce(ExprArgsInputParts) + 'static,
) -> impl IntoView {
    // Initial analysis — used to size dice inputs (dice are assumed static
    // w.r.t. cascade upstream). active_args are read reactively below.
    let initial_analysis = {
        let character = character.get_untracked();
        expr.analyze(&*character, Attribute::arg_index)
    };

    let has_any_args = expr.has_var(|var| matches!(var, Attribute::Arg(_)));
    let has_dice = !initial_analysis.dice_rolls.is_empty();

    let i18n = expect_context::<leptos_fluent::I18n>();

    // Pre-allocate ARG signals up to the largest index the expression can
    // possibly reach. Stable across reactive re-analyses — user input
    // survives cascade rebuilds.
    let arg_slots = expr.arg_slot_count(Attribute::arg_index);
    let arg_signals: Vec<RwSignal<i32>> = (0..arg_slots)
        .map(|i| RwSignal::new(prefill.args.get(i).copied().unwrap_or(0)))
        .collect();
    let read_signals: Vec<Signal<i32>> = arg_signals.iter().copied().map(Into::into).collect();

    // Reactive analysis — re-runs whenever the character snapshot changes.
    let expr_for_analysis = expr.clone();
    let analysis: Memo<ExprAnalysis> = Memo::new(move |_| {
        #[cfg(feature = "perf-marks")]
        let _s = tracing::info_span!("expr_args_input.analysis").entered();
        let character = character.get();
        expr_for_analysis.analyze(&*character, Attribute::arg_index)
    });

    // Zero arg signals at slots that fell out of `analysis.active_args`
    // (cross-source dedup: a pick another source made redundant must not
    // inflate this expression's guard count).
    let arg_signals_for_cleanup = arg_signals.clone();
    Effect::new(move |_| {
        let snapshot = analysis.get();
        for (i, signal) in arg_signals_for_cleanup.iter().enumerate() {
            if snapshot.is_dead_slot(i, signal.get_untracked()) {
                signal.set(0);
            }
        }
    });

    // Dice signals fixed at build time from initial analysis (dice rarely
    // depend on upstream cascade state). dice_signals_cell used by is_valid.
    let dice_signals_cell: RwSignal<Option<(DiceGroupSignals, u32)>> = RwSignal::new(None);

    // Validation Memo — reads character + arg signals reactively.
    let eval_expr = expr.clone();
    let is_valid = Memo::new(move |_| {
        #[cfg(feature = "perf-marks")]
        let _s = tracing::info_span!("expr_args_input.is_valid").entered();
        let args_ok = {
            let character = character.get();
            let ctx = ArgContext {
                character: &character,
                args: &read_signals,
            };
            eval_expr.eval_lenient(&ctx).is_ok()
        };

        let dice_ok = dice_signals_cell
            .read()
            .as_ref()
            .is_none_or(|(groups, total)| {
                let filled: u32 = groups
                    .values()
                    .flat_map(|signals| signals.iter())
                    .filter(|signal| signal.get() > 0)
                    .count() as u32;
                filled == *total
            });

        args_ok && dice_ok
    });

    // Disable untouched inputs only when flipping one to `1` would break the
    // guard. `is_valid` alone is the wrong test for `<= K` caps — it's true
    // at any 0..K, which would lock the picker after the first click.
    let arg_signals_for_probe = arg_signals.clone();
    let probe_expr = expr.clone();
    let is_satisfied = Memo::new(move |_| {
        if !analysis.with(|snapshot| snapshot.has_guard) {
            return false;
        }
        if !is_valid.get() {
            return false;
        }
        let Some(zero_idx) = arg_signals_for_probe.iter().position(|sig| sig.get() == 0) else {
            return false;
        };
        let character = character.get();
        let mut probe_args: Vec<i32> = arg_signals_for_probe.iter().map(|s| s.get()).collect();
        probe_args[zero_idx] = 1;
        let probe_ctx = ProbeContext {
            character: &character,
            args: probe_args,
        };
        probe_expr.eval_lenient(&probe_ctx).is_err()
    });

    // Build dice input groups (fixed from initial_analysis).
    let (dice_signals, dice_groups_el) = if has_dice {
        let total_needed: u32 = initial_analysis.dice_rolls.values().copied().sum();
        let (signals, groups_view) = build_dice_groups(&initial_analysis.dice_rolls, &prefill.dice);
        dice_signals_cell.set(Some((signals.clone(), total_needed)));
        let el = Some(view! { <div class="dice-pool-groups">{groups_view}</div> });
        (signals, el)
    } else {
        (BTreeMap::new(), None)
    };

    // Reactive formula — rebuilds on analysis change. Signals are stable;
    // only the view nodes (disabled / checkbox-vs-number / visible) reflect
    // the latest active_args / boolean_args.
    let expr_for_render = expr.clone();
    let arg_signals_for_render = arg_signals.clone();
    // Formula view: reactively picks between "interactive form" and "no
    // eligible options". When the expr has no ARGs at all (or no active
    // ARGs and no structural ARGs), nothing is rendered — the expression
    // itself is already available via the `ExprDetails` toggle sibling.
    let formula_el = view! {
        {move || {
            let snapshot = analysis.get();
            let has_args = !snapshot.active_args.is_empty();
            if has_args {
                let mut form_ctx = FormCtx::new(
                    &arg_signals_for_render,
                    snapshot.active_args,
                    snapshot.boolean_args,
                    i18n,
                    is_satisfied,
                );
                form_block(&expr_for_render, expr::BLOCK_MAIN, &mut form_ctx, false)
                    .unwrap_or_else(|err| format!("Error: {err}").into_any())
            } else if has_any_args {
                view! { <p class="expr-form-empty">{move_tr!("no-eligible-options")}</p> }
                    .into_any()
            } else {
                ().into_any()
            }
        }}
    };

    on_ready(ExprArgsInputParts {
        arg_signals,
        dice_signals,
        is_valid,
    });

    view! {
        <div class="expr-formula" class:invalid=move || !is_valid.get()>
            {formula_el}
            {dice_groups_el}
        </div>
    }
}
