use std::collections::BTreeSet;

use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    expr::{self, Context, Expr, Op},
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

    fn exec_op(&mut self, op: Op<Attribute, i32>) -> Result<(), expr::Error> {
        match op {
            Op::PushConst(n) => self.push_text(n),
            Op::PushVar(var) => self.push_text(var),
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
                let var_s = var.to_string();
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

fn form_block(
    expr: &Expr<Attribute, i32>,
    block: usize,
    args: &mut Vec<RwSignal<i32>>,
    seen: &mut BTreeSet<u8>,
) -> Result<AnyView, expr::Error> {
    let ops = expr.block(block);

    // Split on statement boundaries (Assign ops) and check each for compound
    // assignment patterns.
    let mut stmts: Vec<&[Op<Attribute, i32>]> = Vec::new();
    let mut start = 0;
    for (i, op) in ops.iter().enumerate() {
        if matches!(op, Op::Assign(_)) {
            stmts.push(&ops[start..=i]);
            start = i + 1;
        }
    }
    if start < ops.len() {
        stmts.push(&ops[start..]);
    }

    let mut fb = FormBuilder::new();
    for stmt in stmts {
        if let Some(ca) = Op::detect_compound(stmt) {
            let assign_var = match stmt.last() {
                Some(Op::Assign(var)) => *var,
                _ => unreachable!(),
            };
            form_block_ops(&mut fb, expr, &stmt[ca.rhs_start..ca.rhs_end], args, seen)?;
            let rhs = fb.pop()?;
            let var_s = assign_var.to_string();
            let sym = ca.sym;
            fb.push_view(view! { <>{var_s}" "{sym}"= "{rhs}</> }.into_any());
        } else {
            form_block_ops(&mut fb, expr, stmt, args, seen)?;
        }
    }
    fb.finish()
}

fn form_block_ops(
    fb: &mut FormBuilder,
    expr: &Expr<Attribute, i32>,
    ops: &[Op<Attribute, i32>],
    args: &mut Vec<RwSignal<i32>>,
    seen: &mut BTreeSet<u8>,
) -> Result<(), expr::Error> {
    for &op in ops {
        match op {
            Op::PushVar(Attribute::Arg(n)) => {
                let idx = n as usize;
                if args.len() <= idx {
                    args.resize_with(idx + 1, || RwSignal::new(0));
                }
                let signal = args[idx];
                if seen.insert(n) {
                    fb.push_view(
                        view! {
                            <input
                                type="number"
                                class="expr-form-input"
                                prop:value=move || signal.get()
                                on:change=move |ev| {
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
                let sub = form_block(expr, idx as usize, args, seen)?;
                fb.push_view(sub);
            }
            Op::EvalIf(then_idx, else_idx) => {
                let cond = fb.pop()?;
                let then_view = form_block(expr, then_idx as usize, args, seen)?;
                if else_idx == 0 {
                    fb.push_view(view! { <>"if("{cond}", "{then_view}")"</> }.into_any());
                } else {
                    let else_view = form_block(expr, else_idx as usize, args, seen)?;
                    fb.push_view(
                        view! { <>"if("{cond}", "{then_view}", "{else_view}")"</> }.into_any(),
                    );
                }
            }
            op => fb.exec_op(op)?,
        }
    }
    Ok(())
}

// --- ExprArgsInput ---

/// The rendered parts of an expression args input: arg signals and validation
/// memo. Returned by `ExprArgsInput` via the `on_ready` callback so the parent
/// can wire up a shared submit button.
pub struct ExprArgsInputParts {
    pub rw_signals: Vec<RwSignal<i32>>,
    pub is_valid: Memo<bool>,
}

/// Renders the interactive formula with number inputs for `ARG.n` variables.
/// No submit button — the parent is responsible for submission. Calls
/// `on_ready` synchronously during build with the signals and validation memo.
#[component]
pub fn ExprArgsInput(
    expr: Expr<Attribute, i32>,
    on_ready: impl FnOnce(ExprArgsInputParts) + 'static,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    // Detect if(cond, then) pattern: Eval(cond_id) EvalIf(then_id, 0)
    let Some((cond_id, then_id)) = expr.windows(2).find_map(|w| match w {
        [Op::Eval(cond), Op::EvalIf(then, 0)] => Some((*cond as usize, *then as usize)),
        _ => None,
    }) else {
        on_ready(ExprArgsInputParts {
            rw_signals: Vec::new(),
            is_valid: Memo::new(|_| true),
        });
        return view! { <span class="expr-form-plain">{expr.to_string()}</span> }.into_any();
    };

    let mut args = Vec::new();
    let mut seen = BTreeSet::new();
    let formula_view = form_block(&expr, then_id, &mut args, &mut seen)
        .unwrap_or_else(|e| format!("Error: {e}").into_any());

    let arg_signals: Vec<Signal<i32>> = args.iter().map(|s| (*s).into()).collect();
    let arg_signals_stored = StoredValue::new(arg_signals);

    let cond_expr = expr.clone();
    let is_valid = Memo::new(move |_| {
        let character = store.read();
        arg_signals_stored.with_value(|sigs| {
            let ctx = ArgContext {
                character: &character,
                args: sigs,
            };
            cond_expr.eval_block(cond_id, &ctx).is_ok_and(|v| v != 0)
        })
    });

    // Render condition with same arg signals (all marked seen → read-only spans)
    let mut cond_args = args.clone();
    let mut cond_seen = seen;
    let cond_view = form_block(&expr, cond_id, &mut cond_args, &mut cond_seen)
        .unwrap_or_else(|e| format!("Error: {e}").into_any());

    on_ready(ExprArgsInputParts {
        rw_signals: args,
        is_valid,
    });

    view! {
        <div class="expr-formula">{formula_view}</div>
        <div class="expr-condition" class:invalid=move || !is_valid.get()>
            <span class="expr-condition-result">
                {move || if is_valid.get() { "\u{2713}" } else { "\u{2717}" }}
            </span>
            {cond_view}
        </div>
    }
    .into_any()
}
