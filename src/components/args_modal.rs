use std::collections::BTreeMap;

use leptos::prelude::*;
use leptos_fluent::move_tr;

use crate::{
    components::{
        expr_args_input::{DiceGroupSignals, ExprArgsInput, ExprArgsInputParts, collect_dice_pool},
        expr_view::ExprDetails,
        modal::Modal,
    },
    expr::DicePool,
    model::AssignInputs,
    rules::{ApplyInputs, PendingInputs},
};

type ArgsCallback = Box<dyn Fn(ApplyInputs) + Send + Sync>;
type ArgsSignals = Vec<(String, Vec<StoredValue<Vec<RwSignal<i32>>>>)>;
type DiceSignals = Vec<(String, Vec<StoredValue<DiceGroupSignals>>)>;

/// Context provided in `CharacterLayout` so any child component can trigger
/// the args-collection modal before applying a feature.
#[derive(Clone, Copy)]
pub struct ArgsModalCtx {
    show: RwSignal<bool>,
    pending: RwSignal<Vec<PendingInputs>>,
    callback: StoredValue<RwSignal<Option<StoredValue<ArgsCallback>>>>,
}

impl ArgsModalCtx {
    pub fn new() -> Self {
        Self {
            show: RwSignal::new(false),
            pending: RwSignal::new(Vec::new()),
            callback: StoredValue::new(RwSignal::new(None)),
        }
    }

    /// Show the modal for a list of features needing interaction. When the user
    /// submits, `on_complete` is called with the collected `ApplyInputs`.
    pub fn open(
        &self,
        pending: Vec<PendingInputs>,
        on_complete: impl Fn(ApplyInputs) + Send + Sync + 'static,
    ) {
        self.pending.set(pending);
        self.callback
            .with_value(|sig| sig.set(Some(StoredValue::new(Box::new(on_complete)))));
        self.show.set(true);
    }

    fn complete(&self, inputs: ApplyInputs) {
        self.callback.with_value(|signal| {
            if let Some(callback) = signal.get_untracked() {
                callback.with_value(|on_complete| on_complete(inputs));
            }
            signal.set(None);
        });
        self.show.set(false);
    }
}

#[component]
fn ArgsFeatureInput(
    pending_inputs: PendingInputs,
    all_signals: RwSignal<ArgsSignals>,
    all_dice: RwSignal<DiceSignals>,
    all_valid: RwSignal<Vec<Memo<bool>>>,
) -> impl IntoView {
    let feature_name = pending_inputs.feature_name.clone();
    let description = pending_inputs.feature_description.clone();
    let has_description = !description.is_empty();

    // Collect signal groups for all exprs of this feature
    let signal_groups: StoredValue<Vec<StoredValue<Vec<RwSignal<i32>>>>> =
        StoredValue::new(Vec::new());
    let dice_groups: StoredValue<Vec<StoredValue<DiceGroupSignals>>> = StoredValue::new(Vec::new());
    let name_for_signals = feature_name.clone();
    let name_for_dice = feature_name.clone();

    let expr_views = pending_inputs
        .exprs
        .into_iter()
        .map(|expr| {
            let on_ready = move |parts: ExprArgsInputParts| {
                signal_groups.update_value(|groups| {
                    groups.push(StoredValue::new(parts.arg_signals));
                });
                dice_groups.update_value(|groups| {
                    groups.push(StoredValue::new(parts.dice_signals));
                });
                all_valid.update(|validations| validations.push(parts.is_valid));
            };
            view! {
                <ExprDetails expr=expr.clone() />
                <ExprArgsInput expr on_ready />
            }
        })
        .collect_view();

    // Register all signal groups for this feature after building
    all_signals.update(|signals| {
        signal_groups.with_value(|groups| {
            signals.push((name_for_signals.clone(), groups.clone()));
        });
    });
    all_dice.update(|dice| {
        dice_groups.with_value(|groups| {
            dice.push((name_for_dice.clone(), groups.clone()));
        });
    });

    view! {
        <div class="args-modal-feature">
            <h4>{pending_inputs.feature_label.clone()}</h4>
            <Show when=move || has_description>
                <p class="args-modal-description">{description.clone()}</p>
            </Show>
            {expr_views}
        </div>
    }
}

#[component]
pub fn ArgsModal() -> impl IntoView {
    let ctx = expect_context::<ArgsModalCtx>();

    let title = Signal::derive(move || move_tr!("apply-features-title").get());

    view! {
        <Modal show=ctx.show title=title>
            {move || {
                let pending = ctx.pending.get();
                if pending.is_empty() {
                    return None;
                }

                let all_signals: RwSignal<ArgsSignals> = RwSignal::new(Vec::new());
                let all_dice: RwSignal<DiceSignals> = RwSignal::new(Vec::new());
                let all_valid: RwSignal<Vec<Memo<bool>>> = RwSignal::new(Vec::new());

                let feature_views = pending
                    .into_iter()
                    .map(|pending_inputs| {
                        view! { <ArgsFeatureInput pending_inputs all_signals all_dice all_valid /> }
                    })
                    .collect_view();

                let is_valid = Memo::new(move |_| {
                    all_valid.with(|validations| {
                        !validations.is_empty()
                            && validations.iter().all(|memo| memo.get())
                    })
                });

                let on_submit = move |event: web_sys::SubmitEvent| {
                    event.prevent_default();
                    let mut inputs_map: BTreeMap<String, Vec<AssignInputs>> = BTreeMap::new();

                    all_signals.with_untracked(|entries| {
                        for (name, groups) in entries {
                            let feature_inputs: Vec<AssignInputs> = groups
                                .iter()
                                .map(|sigs| {
                                    let args = sigs.with_value(|signals| {
                                        signals.iter().map(|signal| signal.get_untracked()).collect()
                                    });
                                    AssignInputs {
                                        args,
                                        dice: DicePool::default(),
                                    }
                                })
                                .collect();
                            inputs_map.insert(name.clone(), feature_inputs);
                        }
                    });

                    all_dice.with_untracked(|entries| {
                        for (name, groups) in entries {
                            let feature_inputs = inputs_map.entry(name.clone()).or_default();
                            for (i, dice_signals) in groups.iter().enumerate() {
                                let dice = dice_signals.with_value(collect_dice_pool);
                                if i < feature_inputs.len() {
                                    feature_inputs[i].dice = dice;
                                } else {
                                    feature_inputs.push(AssignInputs {
                                        args: Vec::new(),
                                        dice,
                                    });
                                }
                            }
                        }
                    });

                    ctx.complete(ApplyInputs(inputs_map));
                };

                Some(
                    view! {
                        <form class="args-modal-body" on:submit=on_submit>
                            {feature_views}
                            <button type="submit" class="btn-primary" disabled=move || !is_valid.get()>
                                {move_tr!("apply-features-title")}
                            </button>
                        </form>
                    }
                    .into_any(),
                )
            }}
        </Modal>
    }
}
