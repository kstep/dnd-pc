use std::{collections::BTreeMap, sync::Arc};

use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{
        datalist_input::{DatalistInput, DatalistOption},
        expr_args_input::{DiceGroupSignals, ExprArgsInput, ExprArgsInputParts, collect_dice_pool},
        expr_view::ExprDetails,
        modal::Modal,
    },
    expr::DicePool,
    model::{AssignInputs, Character, Expr, FeatureSource},
    rules::{ApplyInputs, FeatureKey, PendingInputs, ReplaceWith, RulesRegistry, WhenCondition},
};

type ArgsCallback = Box<dyn FnOnce(ApplyInputs) + Send + Sync>;
type ArgsSignals = BTreeMap<FeatureKey, Vec<StoredValue<Vec<RwSignal<i32>>>>>;
type DiceSignals = BTreeMap<FeatureKey, Vec<StoredValue<DiceGroupSignals>>>;

/// Context provided in `CharacterLayout` so any child component can trigger
/// the args-collection modal before applying a feature.
#[derive(Clone, Copy)]
pub struct ArgsModalCtx {
    show: RwSignal<bool>,
    pending: RwSignal<Vec<PendingInputs>>,
    callback: StoredValue<Option<ArgsCallback>>,
}

impl ArgsModalCtx {
    pub fn new() -> Self {
        Self {
            show: RwSignal::new(false),
            pending: RwSignal::new(Vec::new()),
            callback: StoredValue::new(None),
        }
    }

    /// Show the modal for a list of features needing interaction. When the user
    /// submits, `on_complete` is called once with the collected `ApplyInputs`.
    pub fn open(
        &self,
        pending: Vec<PendingInputs>,
        on_complete: impl FnOnce(ApplyInputs) + Send + Sync + 'static,
    ) {
        self.pending.set(pending);
        self.callback
            .update_value(|cb| *cb = Some(Box::new(on_complete)));
        self.show.set(true);
    }

    fn complete(&self, inputs: ApplyInputs) {
        self.callback.update_value(|cb| {
            if let Some(callback) = cb.take() {
                callback(inputs);
            }
        });
        self.show.set(false);
    }
}

#[component]
fn ArgsFeatureInput(
    pending_inputs: PendingInputs,
    /// Snapshot of the character BEFORE this feature is applied — the
    /// cascade of preceding pending features applied to
    /// `Character::default()` seeded with live identity. Drives the
    /// `ExprArgsInput` analysis so downstream features see upstream's
    /// expression effects.
    character: Signal<Arc<Character>>,
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
    let replace_only = pending_inputs.is_replace_only();
    let source = pending_inputs.source.clone();
    let prefilled_replacement = pending_inputs.prefilled_replacement.clone();
    let replacement_prefill = pending_inputs.replacement_prefill.clone();

    // Signal tracking whether user chose to replace this feature.
    // Pre-filled from AI generation if present — user can still override.
    let replacement_choice: RwSignal<Option<String>> = RwSignal::new(prefilled_replacement);
    if replaceable {
        all_replacements.update(|map| {
            map.insert(feature_name.clone(), replacement_choice);
        });
    }

    // Collect signal groups for all exprs of this feature
    let signal_groups: StoredValue<Vec<StoredValue<Vec<RwSignal<i32>>>>> =
        StoredValue::new(Vec::new());
    let dice_groups: StoredValue<Vec<StoredValue<DiceGroupSignals>>> = StoredValue::new(Vec::new());
    let key = FeatureKey::new(feature_name, source.clone());

    // For replaceable features, collect expr validity locally so we can
    // bypass it when the user picks a replacement.
    let expr_valids: RwSignal<Vec<Memo<bool>>> = RwSignal::new(Vec::new());

    let prefill = pending_inputs.prefill.clone();
    let expr_views = pending_inputs
        .exprs
        .into_iter()
        .enumerate()
        .map(|(i, expr)| {
            let prefill = prefill.get(i).cloned().unwrap_or_default();
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
                <ExprArgsInput expr character prefill on_ready />
            }
        })
        .collect_view();

    // Register all signal groups for this feature after building
    all_signals.update(|signals| {
        signal_groups.with_value(|groups| {
            signals.insert(key.clone(), groups.clone());
        });
    });
    all_dice.update(|dice| {
        dice_groups.with_value(|groups| {
            dice.insert(key.clone(), groups.clone());
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
                view! { <ReplacementPicker replace_with replacement_choice replacement_prefill character all_signals all_dice all_valid source replace_only /> }
            })}
        </div>
    }
}

#[component]
fn ReplacementPicker(
    replace_with: ReplaceWith,
    replacement_choice: RwSignal<Option<String>>,
    /// Pre-filled ARG values for the replacement feature's expressions (AI
    /// generation). Broadcast to every interactive expr of the chosen
    /// replacement — same semantics as the non-replacement `prefill`.
    replacement_prefill: Option<AssignInputs>,
    /// Snapshot of the character BEFORE the original feature (the one
    /// being replaced) was applied.
    character: Signal<Arc<Character>>,
    all_signals: RwSignal<ArgsSignals>,
    all_dice: RwSignal<DiceSignals>,
    all_valid: RwSignal<Vec<Memo<bool>>>,
    source: FeatureSource,
    replace_only: bool,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();
    let initial_replacement = replacement_choice.get_untracked();
    let replacing = RwSignal::new(replace_only || initial_replacement.is_some());
    let replacement_prefill = StoredValue::new(replacement_prefill);
    let source = StoredValue::new(source);

    let options = Signal::derive(move || {
        let character = store.read();
        registry.with_features_index(|features_index| {
            features_index
                .values()
                .filter(|feat| replace_with.matches(feat) && feat.meets_prerequisites(&character))
                .map(|feat| DatalistOption::new(&feat.name, feat.label(), &feat.description))
                .collect::<Vec<_>>()
        })
    });

    let input_value = RwSignal::new(String::new());
    let placeholder = Signal::derive(move || move_tr!("replace-with-feat").get());

    // Expressions for the currently selected replacement feat (if it needs ARGs)
    let replacement_exprs: RwSignal<Vec<Expr>> = RwSignal::new(Vec::new());
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

    // Load (description, exprs) for a replacement feature name. Used by both
    // the initial-seed path (pre-filled from AI generation) and by `on_input`
    // when the user selects from the datalist.
    let load_replacement_data = move |name: &str| -> (String, Vec<Expr>) {
        character.with_untracked(|character| {
            let exprs = source.with_value(|source| {
                registry
                    .feature_needs_args(character, name, Some(source))
                    .map(|pending| pending.exprs)
                    .unwrap_or_default()
            });
            let description = registry.with_features_index(|idx| {
                idx.get(name)
                    .map(|feat| feat.description.clone())
                    .unwrap_or_default()
            });
            (description, exprs)
        })
    };

    let on_input = move |text: String, resolved: Option<String>| {
        let prev = prev_replacement.get_untracked();
        let selection_changed = prev != resolved;

        // AI-seeded prefill is meaningful only for the AI-chosen replacement.
        // Any user switch to a different choice invalidates it. A no-op
        // re-select of the same name preserves the prefill.
        if selection_changed {
            replacement_prefill.set_value(None);
        }

        // Clean up stale signal/dice entries from previous replacement
        if let Some(old_name) = prev {
            all_signals.update(|entries| entries.retain(|key, _| key.name != old_name));
            all_dice.update(|entries| entries.retain(|key, _| key.name != old_name));
        }
        replacement_valids.set(Vec::new());

        input_value.set(text);
        if let Some(name) = &resolved {
            let (description, exprs) = load_replacement_data(name);
            replacement_description.set(description);
            replacement_exprs.set(exprs);
        } else {
            replacement_description.set(String::new());
            replacement_exprs.set(Vec::new());
        }
        replacement_choice.set(resolved.clone());
        prev_replacement.set(resolved);
    };

    // Seed state from pre-filled replacement (e.g. AI generation). Runs once
    // at mount; subsequent user interaction goes through `on_input`.
    if let Some(name) = initial_replacement {
        let label = registry.with_features_index(|idx| {
            idx.get(name.as_str())
                .map(|feat| feat.label().to_string())
                .unwrap_or_else(|| name.clone())
        });
        let (description, exprs) = load_replacement_data(&name);
        input_value.set(label);
        replacement_description.set(description);
        replacement_exprs.set(exprs);
        prev_replacement.set(Some(name));
    }

    view! {
        <div class="replacement-picker">
            <label class="replacement-toggle">
                <input
                    type="checkbox"
                    prop:checked=replacing
                    prop:disabled=replace_only
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
                    required=true
                />
                <Show when=move || !replacement_description.with(String::is_empty)>
                    <p class="args-modal-description">{move || replacement_description.get()}</p>
                </Show>
                {move || {
                    let exprs = replacement_exprs.get();
                    let feat_name = replacement_choice.get()?;
                    if exprs.is_empty() {
                        return None;
                    }

                    let signal_groups: StoredValue<Vec<StoredValue<Vec<RwSignal<i32>>>>> =
                        StoredValue::new(Vec::new());
                    let dice_groups: StoredValue<Vec<StoredValue<DiceGroupSignals>>> =
                        StoredValue::new(Vec::new());
                    let key = FeatureKey::new(feat_name, source.get_value());

                    let expr_views: Vec<_> = exprs
                        .into_iter()
                        .map(|expr| {
                            let prefill = replacement_prefill.get_value().unwrap_or_default();
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
                                <ExprArgsInput expr character prefill on_ready />
                            }
                        })
                        .collect();

                    all_signals.update(|signals| {
                        signal_groups.with_value(|groups| {
                            signals.insert(key.clone(), groups.clone());
                        });
                    });
                    all_dice.update(|dice| {
                        dice_groups.with_value(|groups| {
                            dice.insert(key.clone(), groups.clone());
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

                let all_signals: RwSignal<ArgsSignals> = RwSignal::new(BTreeMap::new());
                let all_dice: RwSignal<DiceSignals> = RwSignal::new(BTreeMap::new());
                let all_valid: RwSignal<Vec<Memo<bool>>> = RwSignal::new(Vec::new());
                let all_replacements: RwSignal<BTreeMap<String, RwSignal<Option<String>>>> =
                    RwSignal::new(BTreeMap::new());

                // Build cascade chain: snapshot[i] = character with pending
                // features 0..i applied (via FeatureDefinition::assign — the
                // expression-only subset of apply, sufficient for analyze).
                // snapshot[0] = `Character::default()` seeded with the live
                // identity (classes/level/species) so derived attributes
                // like PROF_BONUS / CLASS_LEVEL resolve correctly.
                let store = expect_context::<Store<Character>>();
                let registry = expect_context::<RulesRegistry>();
                // Seed from the live character (cloned once at modal open) so
                // snapshots[0] reflects all features already applied to the
                // sheet. Subsequent per-tick cascades clone this Arc's target
                // — not the live store — keeping reactivity isolated.
                //
                // INVARIANT: this seed is intentionally one-shot, never re-synced
                // with `store`. Modal blocks the UI so user mutations cannot
                // happen concurrently. Background writes (auto-save `updated_at`,
                // cloud sync) currently do not touch any Attribute the cascade
                // expressions resolve, so staleness is harmless. If a future
                // background path mutates analysis-relevant Attributes while the
                // modal is open, subscribe to `store` here — expect extra
                // cascade ticks on every unrelated store write.
                let shared_base: Arc<Character> = Arc::new(store.get_untracked());

                let feature_keys: Vec<FeatureKey> = pending
                    .iter()
                    .map(|pending_input| {
                        FeatureKey::new(
                            pending_input.feature_name.clone(),
                            pending_input.source.clone(),
                        )
                    })
                    .collect();

                // `Character` doesn't implement PartialEq (by CLAUDE.md
                // convention — too many non-comparable fields), so Memo is
                // off the table. Use RwSignal<Arc<Character>> + Effects:
                // snapshot[0] is the fixed base; snapshot[i+1] is written
                // by an Effect that reads snapshot[i] and the current
                // inputs for pending[i], applies assign(), and writes the
                // resulting Arc<Character>.
                let snapshots: Vec<RwSignal<Arc<Character>>> = (0..=pending.len())
                    .map(|_| RwSignal::new(shared_base.clone()))
                    .collect();

                for i in 0..pending.len() {
                    let prev_sig = snapshots[i];
                    let next_sig = snapshots[i + 1];
                    let key = feature_keys[i].clone();
                    Effect::new(move |_| {
                        let prev = prev_sig.get();
                        // TODO(perf): per-keystroke cascade does N full
                        // Character clones (N = pending.len()). In realistic
                        // modal scenarios N is small (edit single = 1,
                        // level-up = 1-3, quick-start ≤ 10), so cost is
                        // negligible. Revisit if we ever open a modal with
                        // the full character chain (e.g. ~30 pending features
                        // at Rogue 20 via `collect_rebuild_pending_inputs`)
                        // — if ticks become sluggish, bench first. Candidates:
                        // partial snapshot (skills/abilities/proficiencies
                        // only), or structural sharing via a shadow overlay.
                        let mut ch = (*prev).clone();
                        // Effective feature: user-picked replacement (reactive)
                        // or the original. Replacement's args/dice are stored
                        // under its own FeatureKey in all_signals/all_dice.
                        let replacement = all_replacements.with(|map| {
                            map.get(&key.name).and_then(|sig| sig.get())
                        });
                        let effective_key = match &replacement {
                            Some(name) => FeatureKey::new(name.clone(), key.source.clone()),
                            None => key.clone(),
                        };
                        let dice_pools: Vec<DicePool> = all_dice.with(|entries| {
                            entries
                                .get(&effective_key)
                                .map(|groups| {
                                    groups
                                        .iter()
                                        .map(|dice_group| {
                                            dice_group.with_value(collect_dice_pool)
                                        })
                                        .collect()
                                })
                                .unwrap_or_default()
                        });
                        let inputs: Vec<AssignInputs> = all_signals.with(|entries| {
                            entries
                                .get(&effective_key)
                                .map(|groups| {
                                    groups
                                        .iter()
                                        .enumerate()
                                        .map(|(i, sig_group)| {
                                            sig_group.with_value(|signals| AssignInputs {
                                                args: signals
                                                    .iter()
                                                    .map(|signal| signal.get())
                                                    .collect(),
                                                dice: dice_pools
                                                    .get(i)
                                                    .cloned()
                                                    .unwrap_or_default(),
                                            })
                                        })
                                        .collect()
                                })
                                .unwrap_or_default()
                        });
                        registry.with_features_index_untracked(|idx| {
                            if let Some(def) = idx.get(effective_key.name.as_str()) {
                                // `assign_silent`: cascade is a preview; inputs
                                // may be empty mid-interaction, so `@ARG`
                                // resolution failures / guard-failures are
                                // expected. Real apply uses `assign` which logs.
                                def.assign_silent(
                                    &mut ch,
                                    WhenCondition::OnFeatureAdd,
                                    &inputs,
                                );
                                // OnCompute picks up derived state (AC, initiative,
                                // spell DC, etc.) so downstream features' analysis
                                // sees current snapshot's computed values.
                                // Idempotent within a single snapshot — safe to
                                // run per-feature as a stand-in for a global
                                // `registry.compute()` pass. OnCompute assigns
                                // don't use `@ARG` (they read derived state from
                                // character fields), so empty inputs mirror
                                // `registry.assign(OnCompute)` semantics.
                                def.assign_silent(&mut ch, WhenCondition::OnCompute, &[]);
                            }
                        });
                        next_sig.set(Arc::new(ch));
                    });
                }

                let feature_views = pending
                    .into_iter()
                    .enumerate()
                    .map(|(i, pending_inputs)| {
                        let character: Signal<Arc<Character>> = snapshots[i].into();
                        view! { <ArgsFeatureInput pending_inputs character all_signals all_dice all_valid all_replacements /> }
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
                    let replacements: BTreeMap<String, String> = all_replacements.with_untracked(
                        |entries| {
                            entries
                                .iter()
                                .filter_map(|(original_name, signal)| {
                                    signal
                                        .get_untracked()
                                        .map(|replacement| (original_name.clone(), replacement))
                                })
                                .collect()
                        },
                    );

                    // Build inputs_map in a single pass. `all_signals` and `all_dice`
                    // are guaranteed to have matching keys and group counts (both are
                    // pushed together in `on_ready`, one per expr), so we iterate
                    // signals and look up dice by the same index.
                    let inputs_map: BTreeMap<FeatureKey, Vec<AssignInputs>> = all_signals
                        .with_untracked(|sig_entries| {
                            all_dice.with_untracked(|dice_entries| {
                                sig_entries
                                    .iter()
                                    .filter(|(key, _)| !replacements.contains_key(&key.name))
                                    .map(|(key, signal_groups)| {
                                        let dice_groups = dice_entries.get(key);
                                        let feature_inputs: Vec<AssignInputs> = signal_groups
                                            .iter()
                                            .enumerate()
                                            .map(|(i, sigs)| {
                                                let args = sigs.with_value(|signals| {
                                                    signals
                                                        .iter()
                                                        .map(|signal| signal.get_untracked())
                                                        .collect()
                                                });
                                                let dice = dice_groups
                                                    .and_then(|groups| groups.get(i))
                                                    .map(|dice_sv| {
                                                        dice_sv.with_value(collect_dice_pool)
                                                    })
                                                    .unwrap_or_default();
                                                AssignInputs { args, dice }
                                            })
                                            .collect();
                                        (key.clone(), feature_inputs)
                                    })
                                    .collect()
                            })
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
