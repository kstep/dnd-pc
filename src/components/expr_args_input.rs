use std::collections::{BTreeMap, BTreeSet};

use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    expr::{self, BLOCK_ERROR, BLOCK_NOOP, Block, Context, DicePool, Expr, Op},
    model::{Attribute, Character},
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

    fn exec_op(
        &mut self,
        op: Op<Attribute, i32>,
        i18n: &leptos_fluent::I18n,
    ) -> Result<(), expr::Error> {
        match op {
            Op::PushConst(n) => self.push_text(n),
            Op::PushVar(var) => self.push_text(var.display_name(i18n)),
            Op::Add => self.binary_op("+")?,
            Op::Sub => self.binary_op("-")?,
            Op::Mul => self.binary_op("*")?,
            Op::DivFloor => self.binary_op("/")?,
            Op::DivCeil => self.binary_op("\\")?,
            Op::Mod => self.binary_op("%")?,
            Op::Min => self.binary_func("min")?,
            Op::Max => self.binary_func("max")?,
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
            Op::And => self.binary_op("and")?,
            Op::Or => self.binary_op("or")?,
            Op::Not => {
                let a = self.pop()?;
                self.0.push(view! { <>"not "{a}</> }.into_any());
            }
            Op::Cmp(cmp) => self.binary_op(cmp.symbol())?,
            Op::Assign(var) => {
                let val = self.pop()?;
                let var_s = var.display_name(i18n);
                self.0.push(view! { <>{var_s}" = "{val}</> }.into_any());
            }
            Op::In => {
                let c = self.pop()?;
                let b = self.pop()?;
                let a = self.pop()?;
                self.0
                    .push(view! { <>"in("{a}", "{b}", "{c}")"</> }.into_any());
            }
            Op::Eval(_) | Op::EvalIf(_, _) => {} // intercepted by form_block
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
    i18n: leptos_fluent::I18n,
}

impl FormCtx {
    fn new(active_args: Vec<u8>, i18n: leptos_fluent::I18n) -> Self {
        Self {
            args: Vec::new(),
            seen: BTreeSet::new(),
            active_args: active_args.into_iter().collect(),
            i18n,
        }
    }

    fn is_active(&self, n: u8) -> bool {
        self.active_args.contains(&n)
    }
}

fn form_block(
    expr: &Expr<Attribute, i32>,
    block: expr::BlockIndex,
    ctx: &mut FormCtx,
    condition: bool,
) -> Result<AnyView, expr::Error> {
    let block = expr.block(block);

    let mut fb = FormBuilder::new();
    for stmt in block.statements() {
        if let Some(ca) = Block::detect_compound(stmt) {
            let assign_var = match stmt.last() {
                Some(Op::Assign(var)) => *var,
                _ => unreachable!(),
            };
            form_block_ops(
                &mut fb,
                expr,
                &stmt[ca.rhs_start..ca.rhs_end],
                ctx,
                condition,
            )?;
            let rhs = fb.pop()?;
            let var_s = assign_var.display_name(&ctx.i18n);
            let sym = ca.sym;
            fb.push_view(view! { <>{var_s}" "{sym}"= "{rhs}</> }.into_any());
        } else {
            form_block_ops(&mut fb, expr, stmt, ctx, condition)?;
        }
    }
    fb.finish()
}

fn form_block_ops(
    fb: &mut FormBuilder,
    expr: &Expr<Attribute, i32>,
    ops: &[Op<Attribute, i32>],
    ctx: &mut FormCtx,
    condition: bool,
) -> Result<(), expr::Error> {
    for &op in ops {
        match op {
            Op::PushVar(Attribute::Arg(n)) => {
                let idx = n as usize;
                if ctx.args.len() <= idx {
                    ctx.args.resize_with(idx + 1, || RwSignal::new(0));
                }
                let signal = ctx.args[idx];
                // In condition context or inactive ARG: always a ref.
                // In body context + active + first occurrence: input.
                if !condition && ctx.is_active(n) && ctx.seen.insert(n) {
                    fb.push_view(
                        view! {
                            <input
                                type="number"
                                class="expr-form-input"
                                prop:value=move || signal.get()
                                on:input=move |ev| {
                                    let value = event_target_value(&ev).parse::<i32>().unwrap_or(0);
                                    signal.set(value);
                                }
                            />
                        }
                        .into_any(),
                    );
                } else {
                    fb.push_view(
                        view! { <span class="expr-form-ref">{move || signal.get()}</span> }
                            .into_any(),
                    );
                }
            }
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
                    continue;
                }

                let then_view = form_block(expr, then_idx, ctx, false)?;
                if else_idx == BLOCK_NOOP || else_idx == BLOCK_ERROR {
                    fb.push_view(then_view);
                } else if else_has_args {
                    let else_view = form_block(expr, else_idx, ctx, false)?;
                    fb.push_view(
                        view! { <>"if("{cond}", "{then_view}", "{else_view}")"</> }.into_any(),
                    );
                } else {
                    fb.push_view(view! { <>"if("{cond}", "{then_view}")"</> }.into_any());
                }
            }
            op => fb.exec_op(op, &ctx.i18n)?,
        }
    }
    Ok(())
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

fn is_arg(var: &Attribute) -> Option<u8> {
    match var {
        Attribute::Arg(n) => Some(*n),
        _ => None,
    }
}

/// Build dice input groups view from roll requirements.
/// Returns the signal map and the rendered view.
fn build_dice_groups(dice_rolls: &BTreeMap<u32, u32>) -> (DiceGroupSignals, AnyView) {
    let groups: DiceGroupSignals = dice_rolls
        .iter()
        .map(|(&sides, &count)| {
            let signals: Vec<_> = (0..count).map(|_| RwSignal::new(0u32)).collect();
            (sides, signals)
        })
        .collect();

    let mut first = true;
    let view = groups
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
        .collect_view()
        .into_any();

    (groups, view)
}

/// Renders the interactive formula with number inputs for `ARG.n` variables
/// and dice input groups for any dice rolls in the expression.
/// No submit button — the parent is responsible for submission. Calls
/// `on_ready` synchronously during build with the signals and validation memo.
#[component]
pub fn ExprArgsInput(
    expr: Expr<Attribute, i32>,
    on_ready: impl FnOnce(ExprArgsInputParts) + 'static,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    // Analyze: determine which ARGs are reachable and which dice are needed
    let analysis = {
        let character = store.read_untracked();
        expr.analyze(&*character, is_arg)
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

    // Build formula view with inline ARG inputs (if any)
    let formula_view = if has_args {
        let mut form_ctx = FormCtx::new(analysis.active_args, i18n);
        let view = form_block(&expr, expr::BLOCK_MAIN, &mut form_ctx, false)
            .unwrap_or_else(|err| format!("Error: {err}").into_any());

        let read_signals: Vec<Signal<i32>> = form_ctx.args.iter().map(|s| (*s).into()).collect();
        let write_signals = form_ctx.args;

        Some((view, read_signals, write_signals))
    } else {
        None
    };

    let arg_signals = formula_view
        .as_ref()
        .map(|(_, _, write_signals)| write_signals.clone())
        .unwrap_or_default();
    let read_signals_stored = formula_view
        .as_ref()
        .map(|(_, read_signals, _)| StoredValue::new(read_signals.clone()));

    let formula_el = formula_view.map(|(view, _, _)| view);

    // Build dice input groups (if any)
    let (dice_signals, dice_groups_el, dice_signals_stored, dice_total_needed) = if has_dice {
        let total_needed: u32 = analysis.dice_rolls.values().copied().sum();
        let (signals, groups_view) = build_dice_groups(&analysis.dice_rolls);
        let signals_stored = StoredValue::new(signals.clone());
        let el = Some(view! { <div class="dice-pool-groups">{groups_view}</div> });
        (signals, el, Some(signals_stored), total_needed)
    } else {
        (BTreeMap::new(), None, None, 0)
    };

    // Validation: all ARG inputs eval OK AND all dice inputs filled.
    // Both args and dice use RwSignals, so this Memo is fully reactive.
    let eval_expr = expr.clone();
    let is_valid = Memo::new(move |_| {
        let args_ok = if let Some(stored) = read_signals_stored {
            let character = store.read();
            stored.with_value(|signals| {
                let ctx = ArgContext {
                    character: &character,
                    args: signals,
                };
                eval_expr.eval_lenient(&ctx).is_ok()
            })
        } else {
            true
        };

        let dice_ok = if let Some(stored) = dice_signals_stored {
            stored.with_value(|groups| {
                let filled: u32 = groups
                    .values()
                    .flat_map(|signals| signals.iter())
                    .filter(|signal| signal.get() > 0)
                    .count() as u32;
                filled == dice_total_needed
            })
        } else {
            true
        };

        args_ok && dice_ok
    });

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
