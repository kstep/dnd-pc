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
            Op::Lt => self.binary_op("<")?,
            Op::Gt => self.binary_op(">")?,
            Op::Le => self.binary_op("<=")?,
            Op::Ge => self.binary_op(">=")?,
            Op::CmpEq => self.binary_op("==")?,
            Op::CmpNe => self.binary_op("!=")?,
            Op::Assign(var) => {
                let val = self.pop()?;
                let var_s = var.to_string();
                self.0.push(view! { <>{var_s}" = "{val}</> }.into_any());
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
        // Join with "; " like Formatter
        let mut parts: Vec<AnyView> = Vec::with_capacity(self.0.len() * 2 - 1);
        for (i, v) in self.0.into_iter().enumerate() {
            if i > 0 {
                parts.push("; ".into_any());
            }
            parts.push(v);
        }
        Ok(parts.collect_view().into_any())
    }
}

// --- form_block: recursive block walker producing views ---

fn form_block(
    expr: &Expr<Attribute, i32>,
    block: usize,
    args: &mut Vec<RwSignal<i32>>,
    seen: &mut BTreeSet<u8>,
) -> Result<AnyView, expr::Error> {
    let mut fb = FormBuilder::new();
    for &op in expr.block(block) {
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
                                    if let Ok(v) = event_target_value(&ev).parse::<i32>() {
                                        signal.set(v);
                                    }
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
    fb.finish()
}

// --- ExprForm component ---

#[component]
pub fn ExprForm(
    expr: Expr<Attribute, i32>,
    on_submit: impl Fn(Vec<i32>) + Send + Sync + 'static,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let on_submit = StoredValue::new(on_submit);

    // Detect if(cond, then) pattern: Eval(cond_id) EvalIf(then_id, 0)
    let Some((cond_id, then_id)) = expr.windows(2).find_map(|w| match w {
        [Op::Eval(cond), Op::EvalIf(then, 0)] => Some((*cond as usize, *then as usize)),
        _ => None,
    }) else {
        // No if-pattern found — render as plain text
        return view! { <span class="expr-form-plain">{expr.to_string()}</span> }.into_any();
    };

    // Build the formula view from the then-block
    let mut args = Vec::new();
    let mut seen = BTreeSet::new();
    let formula_view = form_block(&expr, then_id, &mut args, &mut seen)
        .unwrap_or_else(|e| format!("Error: {e}").into_any());

    let arg_signals: Vec<Signal<i32>> = args.iter().map(|s| (*s).into()).collect();
    let arg_signals = StoredValue::new(arg_signals);
    let rw_signals = StoredValue::new(args);

    // Reactive validation: evaluate cond block whenever arg signals change
    let is_valid = Memo::new(move |_| {
        let character = store.read();
        arg_signals.with_value(|sigs| {
            let ctx = ArgContext {
                character: &character,
                args: sigs,
            };
            expr.eval_block(cond_id, &ctx).is_ok_and(|v| v != 0)
        })
    });

    let submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        rw_signals.with_value(|sigs| {
            let values: Vec<i32> = sigs.iter().map(|s| s.get_untracked()).collect();
            on_submit.with_value(|cb| cb(values));
        });
    };

    view! {
        <form class="expr-form" on:submit=submit>
            <span class="expr-formula">{formula_view}</span>
            <button type="submit" disabled=move || !is_valid.get()>
                "Submit"
            </button>
        </form>
    }
    .into_any()
}
