use std::collections::{BTreeMap, BTreeSet};

use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::icon::Icon,
    expr::{self, BLOCK_ERROR, BLOCK_NOOP, Block, Context, DicePool, IterIndex, VarGroup},
    model::{Attribute, AttributeGroup, Character, Expr, Op},
};

// --- ArgContext: resolves Arg(n) from signals, delegates rest to Character ---

struct ArgContext<'a> {
    character: &'a Character,
    args: &'a [Signal<i32>],
}

impl Context<Attribute, i32> for ArgContext<'_> {
    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        match var {
            Attribute::Arg(n) => Ok(self.args.get(n as usize).map_or(0, |s| s.get())),
            other => self.character.resolve(other),
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
                self.push_view((move || var.display_name(&i18n)).into_any());
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
                let var_s = move || var.display_name(&i18n);
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
            | Op::Next(_)
            | Op::PushGroup(_)
            | Op::AssignGroup(_)
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
            .map(|v| view! { <div class="expr-formula-line">{v}</div> }.into_any())
            .collect_view()
            .into_any())
    }
}

// --- form_block: recursive block walker producing views ---

/// Context for form building: tracks which ARGs are active (from analysis),
/// which have been seen (first occurrence = input, later = read-only ref),
/// and the arg signals.
struct FormCtx {
    args: Vec<RwSignal<i32>>,
    seen: BTreeSet<u8>,
    active_args: BTreeSet<u8>,
    boolean_args: BTreeSet<u8>,
    i18n: leptos_fluent::I18n,
    iter_stack: Vec<IterIndex>,
    is_satisfied: Memo<bool>,
}

impl FormCtx {
    fn new(
        active_args: Vec<u8>,
        boolean_args: BTreeSet<u8>,
        i18n: leptos_fluent::I18n,
        is_satisfied: Memo<bool>,
    ) -> Self {
        Self {
            args: Vec::new(),
            seen: BTreeSet::new(),
            active_args: active_args.into_iter().collect(),
            boolean_args,
            i18n,
            iter_stack: Vec::new(),
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
    ctx: &mut FormCtx,
    condition: bool,
) -> Result<AnyView, expr::Error> {
    let block = expr.block(block);

    let mut fb = FormBuilder::new();
    for stmt in block.statements() {
        render_statement(&mut fb, expr, stmt, ctx, condition)?;
    }
    fb.finish()
}

/// Render a single statement: detect compound assignment (`X += rhs`) and
/// render as `{label} {op}= {rhs}`, or fall back to `form_block_ops`.
fn render_statement(
    fb: &mut FormBuilder,
    expr: &Expr,
    stmt: &[Op],
    ctx: &mut FormCtx,
    condition: bool,
) -> Result<(), expr::Error> {
    if let Some(ca) = Block::detect_compound(stmt) {
        let i18n = ctx.i18n;
        let (real_idx, seq_idx) = ctx.iter_stack.last().copied().unwrap_or((0, 0));
        let var_view: AnyView = match stmt.last() {
            Some(&Op::AssignVar(var)) => (move || var.display_name(&i18n)).into_any(),
            Some(&Op::AssignGroup(grp)) => {
                let idx = if grp.is_companion() {
                    seq_idx
                } else {
                    real_idx
                };
                let var = grp.member(idx).expect("valid index");
                (move || var.display_name(&i18n)).into_any()
            }
            _ => unreachable!(),
        };
        form_block_ops(fb, expr, &stmt[ca.rhs_start..ca.rhs_end], ctx, condition)?;
        let rhs = fb.pop()?;
        let sym = ca.sym;
        fb.push_view(view! { <>{var_view}" "{sym}"= "{rhs}</> }.into_any());
    } else {
        form_block_ops(fb, expr, stmt, ctx, condition)?;
    }
    Ok(())
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
    ctx: &mut FormCtx,
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
            // PushGroup(Arg) → ARG input using sequential index;
            // other groups use real index from iter_stack
            Op::PushGroup(grp) => {
                if let Some(&(real_idx, seq_idx)) = ctx.iter_stack.last() {
                    let idx = if grp.is_companion() {
                        seq_idx
                    } else {
                        real_idx
                    };
                    if let Some(var) = grp.member(idx) {
                        if let Attribute::Arg(n) = var {
                            push_arg_input(fb, ctx, n, condition);
                        } else {
                            let i18n = ctx.i18n;
                            fb.push_view((move || var.display_name(&i18n)).into_any());
                        }
                    }
                }
            }
            Op::AssignGroup(grp) => {
                if let Some(&(real_idx, seq_idx)) = ctx.iter_stack.last() {
                    let idx = if grp.is_companion() {
                        seq_idx
                    } else {
                        real_idx
                    };
                    if let Some(var) = grp.member(idx) {
                        let val = fb.pop()?;
                        let i18n = ctx.i18n;
                        let var_s = move || var.display_name(&i18n);
                        fb.push_view(view! { <>{var_s}" = "{val}</> }.into_any());
                    }
                }
            }
            Op::Next(_) => {} // handled by loop unroll
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
                    // Guard body: inline into current FormBuilder to avoid
                    // nested wrapping (loop items + trailing statements
                    // should all be siblings in the same grid).
                    let body_ops = expr.block(then_idx);
                    form_block_ops(fb, expr, body_ops, ctx, condition)?;
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
fn form_block_loop(
    fb: &mut FormBuilder,
    expr: &Expr,
    subgrp: expr::VarSubgroup<AttributeGroup>,
    body_idx: expr::BlockIndex,
    ctx: &mut FormCtx,
    condition: bool,
) -> Result<(), expr::Error> {
    let body = expr.block(body_idx);
    let body_ops = &**body;
    // Strip trailing Next + EvalIf (loop control ops)
    let content_end = body_ops.len().saturating_sub(2);
    let content = &body_ops[..content_end];
    for (seq_idx, real_idx) in subgrp.real_indices().enumerate() {
        ctx.iter_stack.push((real_idx, seq_idx));
        render_statement(fb, expr, content, ctx, condition)?;
        ctx.iter_stack.pop();
    }
    Ok(())
}

/// Push an ARG input (checkbox, number, or ref) for the given ARG index.
fn push_arg_input(fb: &mut FormBuilder, ctx: &mut FormCtx, n: u8, condition: bool) {
    let idx = n as usize;
    if ctx.args.len() <= idx {
        ctx.args.resize_with(idx + 1, || RwSignal::new(0));
    }
    let signal = ctx.args[idx];
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
/// Returns the signal map and the rendered view.
pub fn build_dice_groups(dice_rolls: &BTreeMap<u32, u32>) -> (DiceGroupSignals, AnyView) {
    let groups: DiceGroupSignals = dice_rolls
        .iter()
        .map(|(&sides, &count)| {
            let signals: Vec<_> = (0..count).map(|_| RwSignal::new(0u32)).collect();
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
            <Icon name="dices" size=14 />
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
    on_ready: impl FnOnce(ExprArgsInputParts) + 'static,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    // Analyze: determine which ARGs are reachable and which dice are needed
    let analysis = {
        let character = store.read_untracked();
        expr.analyze(&*character, Attribute::arg_index)
    };

    let has_args = !analysis.active_args.is_empty();
    let has_dice = !analysis.dice_rolls.is_empty();

    if !has_args && !has_dice {
        let has_any_args = expr.has_var(|v| matches!(v, Attribute::Arg(_)));
        on_ready(ExprArgsInputParts {
            arg_signals: Vec::new(),
            dice_signals: BTreeMap::new(),
            is_valid: Memo::new(move |_| !has_any_args),
        });
        return Either::Left(if has_any_args {
            Either::Left(view! { <p class="expr-form-empty">{move_tr!("no-eligible-options")}</p> })
        } else {
            Either::Right(view! { <span class="expr-form-plain">{expr.to_string()}</span> })
        });
    }

    let i18n = expect_context::<leptos_fluent::I18n>();

    // Lazy signals — populated after form building, read by is_valid Memo.
    let read_signals_cell: RwSignal<Vec<Signal<i32>>> = RwSignal::new(Vec::new());
    let dice_signals_cell: RwSignal<Option<(DiceGroupSignals, u32)>> = RwSignal::new(None);

    // Validation Memo — created before form so it can be passed to FormCtx.
    // Reads signals lazily; they're populated after form_block runs.
    let eval_expr = expr.clone();
    let is_valid = Memo::new(move |_| {
        let signals = read_signals_cell.read();
        let args_ok = if signals.is_empty() {
            true
        } else {
            let character = store.read();
            let ctx = ArgContext {
                character: &character,
                args: &signals,
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

    // Build formula view with inline ARG inputs (if any)
    let formula_view = if has_args {
        let mut form_ctx =
            FormCtx::new(analysis.active_args, analysis.boolean_args, i18n, is_valid);
        let view = form_block(&expr, expr::BLOCK_MAIN, &mut form_ctx, false)
            .unwrap_or_else(|err| format!("Error: {err}").into_any());

        let read_signals: Vec<Signal<i32>> = form_ctx.args.iter().map(|s| (*s).into()).collect();
        let write_signals = form_ctx.args;

        Some((view, read_signals, write_signals))
    } else {
        None
    };

    let (formula_el, arg_signals) = match formula_view {
        Some((view, read_signals, write_signals)) => {
            read_signals_cell.set(read_signals);
            (Some(view), write_signals)
        }
        None => (None, Vec::new()),
    };

    // Build dice input groups (if any)
    let (dice_signals, dice_groups_el) = if has_dice {
        let total_needed: u32 = analysis.dice_rolls.values().copied().sum();
        let (signals, groups_view) = build_dice_groups(&analysis.dice_rolls);
        dice_signals_cell.set(Some((signals.clone(), total_needed)));
        let el = Some(view! { <div class="dice-pool-groups">{groups_view}</div> });
        (signals, el)
    } else {
        (BTreeMap::new(), None)
    };

    on_ready(ExprArgsInputParts {
        arg_signals,
        dice_signals,
        is_valid,
    });

    Either::Right(view! {
        <div class="expr-formula" class:invalid=move || !is_valid.get()>
            {formula_el}
            {dice_groups_el}
        </div>
    })
}
