use std::collections::BTreeMap;

use leptos::{html, prelude::*};
use leptos_fluent::move_tr;

use crate::{
    components::{
        expr_args_input::{ExprArgsInput, collect_dice_pool},
        expr_view::ExprDetails,
        modal::Modal,
    },
    expr::DicePool,
    rules::{ApplyInputs, PendingArgs},
};

type ArgsCallback = Box<dyn Fn(ApplyInputs) + Send + Sync>;
type ArgsSignals = Vec<(String, Vec<StoredValue<Vec<RwSignal<i32>>>>)>;
type DiceRefs = BTreeMap<u32, Vec<NodeRef<html::Input>>>;
type DiceSignals = Vec<(String, Vec<StoredValue<DiceRefs>>)>;

/// Context provided in `CharacterLayout` so any child component can trigger
/// the args-collection modal before applying a feature.
#[derive(Clone, Copy)]
pub struct ArgsModalCtx {
    show: RwSignal<bool>,
    pending: RwSignal<Vec<PendingArgs>>,
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
        pending: Vec<PendingArgs>,
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
    pending_feature: PendingArgs,
    all_signals: RwSignal<ArgsSignals>,
    all_dice: RwSignal<DiceSignals>,
    all_valid: RwSignal<Vec<Memo<bool>>>,
) -> impl IntoView {
    let feature_name = pending_feature.feature_name.clone();
    let description = pending_feature.feature_description.clone();
    let has_description = !description.is_empty();

    // Collect signal groups for all exprs of this feature
    let signal_groups: StoredValue<Vec<StoredValue<Vec<RwSignal<i32>>>>> =
        StoredValue::new(Vec::new());
    let dice_groups: StoredValue<Vec<StoredValue<DiceRefs>>> = StoredValue::new(Vec::new());
    let name_for_signals = feature_name.clone();
    let name_for_dice = feature_name.clone();

    let expr_views = pending_feature
        .exprs
        .into_iter()
        .map(|expr| {
            let on_ready = move |parts: crate::components::expr_args_input::ExprArgsInputParts| {
                signal_groups.update_value(|groups| {
                    groups.push(StoredValue::new(parts.rw_signals));
                });
                dice_groups.update_value(|groups| {
                    groups.push(StoredValue::new(parts.dice_refs));
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
            <h4>{pending_feature.feature_label.clone()}</h4>
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
                    .map(|pending_feature| {
                        view! { <ArgsFeatureInput pending_feature all_signals all_dice all_valid /> }
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
                    let mut args_map: BTreeMap<String, Vec<Vec<i32>>> = BTreeMap::new();
                    let mut dice_map: BTreeMap<String, Vec<DicePool>> = BTreeMap::new();

                    all_signals.with_untracked(|entries| {
                        for (name, groups) in entries {
                            let feature_args: Vec<Vec<i32>> = groups
                                .iter()
                                .map(|sigs| {
                                    sigs.with_value(|signals| {
                                        signals.iter().map(|signal| signal.get_untracked()).collect()
                                    })
                                })
                                .collect();
                            args_map.insert(name.clone(), feature_args);
                        }
                    });

                    all_dice.with_untracked(|entries| {
                        for (name, groups) in entries {
                            let feature_dice: Vec<DicePool> = groups
                                .iter()
                                .map(|refs| {
                                    refs.with_value(|refs| collect_dice_pool(refs).into())
                                })
                                .collect();
                            dice_map.insert(name.clone(), feature_dice);
                        }
                    });

                    ctx.complete(ApplyInputs {
                        args: args_map,
                        dice: dice_map,
                    });
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
