use std::collections::BTreeMap;

use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{
        datalist_input::DatalistInput,
        expr_args_input::{DiceGroupSignals, ExprArgsInput, ExprArgsInputParts, collect_dice_pool},
        expr_view::ExprDetails,
        modal::Modal,
    },
    expr::{DicePool, Expr},
    model::{AssignInputs, Attribute, Character, FeatureSource},
    rules::{ApplyInputs, PendingInputs, ReplaceWith, RulesRegistry},
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
    all_replacements: RwSignal<BTreeMap<String, RwSignal<Option<String>>>>,
) -> impl IntoView {
    let feature_name = pending_inputs.feature_name.clone();
    let description = pending_inputs.feature_description.clone();
    let has_description = !description.is_empty();
    let replace_with = pending_inputs.replace_with;
    let replaceable = pending_inputs.is_replaceable();
    let source = pending_inputs.source.clone();

    // Signal tracking whether user chose to replace this feature
    let replacement_choice: RwSignal<Option<String>> = RwSignal::new(None);
    if replaceable {
        all_replacements.update(|map| {
            map.insert(feature_name.clone(), replacement_choice);
        });
    }

    // Collect signal groups for all exprs of this feature
    let signal_groups: StoredValue<Vec<StoredValue<Vec<RwSignal<i32>>>>> =
        StoredValue::new(Vec::new());
    let dice_groups: StoredValue<Vec<StoredValue<DiceGroupSignals>>> = StoredValue::new(Vec::new());
    let name_for_signals = feature_name.clone();
    let name_for_dice = feature_name.clone();

    // For replaceable features, collect expr validity locally so we can
    // bypass it when the user picks a replacement.
    let expr_valids: RwSignal<Vec<Memo<bool>>> = RwSignal::new(Vec::new());

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
                if replaceable {
                    expr_valids.update(|validations| validations.push(parts.is_valid));
                } else {
                    all_valid.update(|validations| validations.push(parts.is_valid));
                }
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

    // For replaceable features, push a single combined validity memo:
    // valid if (replacing with a chosen feat) OR (not replacing AND all
    // ARG expr memos pass). For replaceable-only features (no exprs),
    // expr_valids is empty so the fallback is always valid.
    if replaceable {
        all_valid.update(|validations| {
            validations.push(Memo::new(move |_| {
                if replacement_choice.get().is_some() {
                    return true;
                }
                expr_valids.with(|memos| memos.is_empty() || memos.iter().all(|memo| memo.get()))
            }));
        });
    }

    let is_replacing = Memo::new(move |_| replacement_choice.get().is_some());

    view! {
        <div class="args-modal-feature">
            <h4>{pending_inputs.feature_label.clone()}</h4>
            <Show when=move || has_description>
                <p class="args-modal-description">{description.clone()}</p>
            </Show>
            <div style:display=move || if is_replacing.get() { "none" } else { "" }>
                {expr_views}
            </div>
            {replaceable.then(|| {
                let source = source.clone();
                view! { <ReplacementPicker replace_with replacement_choice all_signals all_dice all_valid source /> }
            })}
        </div>
    }
}

#[component]
fn ReplacementPicker(
    replace_with: ReplaceWith,
    replacement_choice: RwSignal<Option<String>>,
    all_signals: RwSignal<ArgsSignals>,
    all_dice: RwSignal<DiceSignals>,
    all_valid: RwSignal<Vec<Memo<bool>>>,
    source: FeatureSource,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();
    let replacing = RwSignal::new(false);
    let source = StoredValue::new(source);

    let options = Signal::derive(move || {
        let character = store.read();
        registry.with_features_index(|features_index| {
            features_index
                .values()
                .filter(|feat| replace_with.matches(feat) && feat.meets_prerequisites(&character))
                .map(|feat| {
                    (
                        feat.name.clone(),
                        feat.label().to_string(),
                        feat.description.clone(),
                    )
                })
                .collect::<Vec<_>>()
        })
    });

    let input_value = RwSignal::new(String::new());
    let placeholder = Signal::derive(move || move_tr!("replace-with-feat").get());

    // Expressions for the currently selected replacement feat (if it needs ARGs)
    let replacement_exprs: RwSignal<Vec<Expr<Attribute>>> = RwSignal::new(Vec::new());
    // Description of the selected replacement feat
    let replacement_description: RwSignal<String> = RwSignal::new(String::new());

    // Track previous replacement name to clean up stale entries from
    // all_signals/all_dice when the user switches replacement choice.
    let prev_replacement: RwSignal<Option<String>> = RwSignal::new(None);

    // Local validity memos for replacement feat ARGs, reset on each selection
    // change. One combined memo is pushed to all_valid (below) so stale memos
    // don't accumulate.
    let replacement_valids: RwSignal<Vec<Memo<bool>>> = RwSignal::new(Vec::new());
    all_valid.update(|validations| {
        validations.push(Memo::new(move |_| {
            replacement_valids.with(|memos| memos.is_empty() || memos.iter().all(|memo| memo.get()))
        }));
    });

    let on_input = move |_text: String, resolved: Option<String>| {
        // Clean up stale signal/dice entries from previous replacement
        if let Some(old_name) = prev_replacement.get_untracked() {
            all_signals.update(|entries| entries.retain(|(name, _)| *name != old_name));
            all_dice.update(|entries| entries.retain(|(name, _)| *name != old_name));
        }
        replacement_valids.set(Vec::new());

        replacement_choice.set(resolved.clone());
        prev_replacement.set(resolved.clone());
        if let Some(name) = &resolved {
            input_value.set(name.clone());
            let (description, exprs) = store.with_untracked(|character| {
                let exprs = source.with_value(|source| {
                    registry
                        .feature_needs_args(character, name, Some(source))
                        .map(|pending| pending.exprs)
                        .unwrap_or_default()
                });
                let description = registry.with_features_index(|idx| {
                    idx.get(name.as_str())
                        .map(|feat| feat.description.clone())
                        .unwrap_or_default()
                });
                (description, exprs)
            });
            replacement_description.set(description);
            replacement_exprs.set(exprs);
        } else {
            replacement_description.set(String::new());
            replacement_exprs.set(Vec::new());
        }
    };

    view! {
        <div class="replacement-picker">
            <label class="replacement-toggle">
                <input
                    type="checkbox"
                    prop:checked=replacing
                    on:change=move |ev| {
                        let checked = event_target_checked(&ev);
                        replacing.set(checked);
                        if !checked {
                            replacement_choice.set(None);
                            input_value.set(String::new());
                            replacement_exprs.set(Vec::new());
                        }
                    }
                />
                {move_tr!("replace-with-feat")}
            </label>
            <Show when=move || replacing.get()>
                <DatalistInput
                    value=input_value
                    placeholder=placeholder
                    options=options
                    on_input=on_input
                />
                <Show when=move || !replacement_description.with(String::is_empty)>
                    <p class="args-modal-description">{move || replacement_description.get()}</p>
                </Show>
                {move || {
                    let exprs = replacement_exprs.get();
                    let feat_name = replacement_choice.get();
                    if exprs.is_empty() || feat_name.is_none() {
                        return None;
                    }
                    let feat_name = feat_name.unwrap();

                    let signal_groups: StoredValue<Vec<StoredValue<Vec<RwSignal<i32>>>>> =
                        StoredValue::new(Vec::new());
                    let dice_groups: StoredValue<Vec<StoredValue<DiceGroupSignals>>> =
                        StoredValue::new(Vec::new());
                    let name_for_signals = feat_name.clone();
                    let name_for_dice = feat_name.clone();

                    let expr_views: Vec<_> = exprs
                        .into_iter()
                        .map(|expr| {
                            let on_ready = move |parts: ExprArgsInputParts| {
                                signal_groups.update_value(|groups| {
                                    groups.push(StoredValue::new(parts.arg_signals));
                                });
                                dice_groups.update_value(|groups| {
                                    groups.push(StoredValue::new(parts.dice_signals));
                                });
                                replacement_valids
                                    .update(|validations| validations.push(parts.is_valid));
                            };
                            view! {
                                <ExprDetails expr=expr.clone() />
                                <ExprArgsInput expr on_ready />
                            }
                        })
                        .collect();

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

                    Some(view! { <div class="replacement-args">{expr_views}</div> }.into_any())
                }}
            </Show>
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
                let all_replacements: RwSignal<BTreeMap<String, RwSignal<Option<String>>>> =
                    RwSignal::new(BTreeMap::new());

                let feature_views = pending
                    .into_iter()
                    .map(|pending_inputs| {
                        view! { <ArgsFeatureInput pending_inputs all_signals all_dice all_valid all_replacements /> }
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

                    // Collect replacement decisions
                    let mut replacements: BTreeMap<String, String> = BTreeMap::new();
                    all_replacements.with_untracked(|entries| {
                        for (original_name, signal) in entries {
                            if let Some(replacement_name) = signal.get_untracked() {
                                replacements.insert(original_name.clone(), replacement_name);
                            }
                        }
                    });

                    let mut inputs_map: BTreeMap<String, Vec<AssignInputs>> = BTreeMap::new();

                    all_signals.with_untracked(|entries| {
                        for (name, groups) in entries {
                            if replacements.contains_key(name) {
                                continue;
                            }
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
                            if replacements.contains_key(name) {
                                continue;
                            }
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

                    ctx.complete(ApplyInputs {
                        feature_inputs: inputs_map,
                        replacements,
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
