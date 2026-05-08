use std::{collections::BTreeMap, sync::Arc};

use leptos::prelude::*;
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;

use crate::{
    components::{
        datalist::{DatalistInput, DatalistOption, SharedDatalist, next_datalist_id},
        expr_args_input::{DiceGroupSignals, ExprArgsInput, ExprArgsInputParts, collect_dice_pool},
        expr_view::ExprDetails,
        markdown::Markdown,
        modal::Modal,
    },
    expr::DicePool,
    model::{
        AssignInputs, Character, CharacterCore, Expr, FeatureCategory, FeatureSource, IdentitySlot,
    },
    rules::{
        ApplyInputs, DefinitionStore, FeatureKey, PendingInputs, RecomputePending, ReplaceWith,
        RulesRegistry,
        apply::{PendingFeature, cascade},
    },
};

type ArgsCallback = Box<dyn FnOnce(ApplyInputs) + Send + Sync>;
type ArgsSignals = BTreeMap<FeatureKey, Vec<StoredValue<Vec<RwSignal<i32>>>>>;
type DiceSignals = BTreeMap<FeatureKey, Vec<StoredValue<DiceGroupSignals>>>;

/// Reactive bundle of args-modal input signals. `Copy` — four RwSignals
/// inside, all owned by the `ArgsModal` component scope. Methods read
/// through `with`/`with_untracked` (controlled by `tracked` param) and
/// never escape references; closures returning these stay synchronous
/// inside one `cascade()` call.
#[derive(Clone, Copy)]
pub struct ArgsModalState {
    pub args: RwSignal<ArgsSignals>,
    pub dice: RwSignal<DiceSignals>,
    pub valid: RwSignal<BTreeMap<FeatureKey, Memo<bool>>>,
    pub replacements: RwSignal<BTreeMap<String, RwSignal<Option<String>>>>,
    /// Per-section downstream snapshots: `chain[K]` is the character after
    /// the section keyed by `K` finishes its cascade. The section that
    /// follows `K` reads it as its upstream. Each section owns its own
    /// `RwSignal<Arc<CharacterCore>>` and removes itself on unmount.
    pub chain: RwSignal<BTreeMap<FeatureKey, RwSignal<Arc<CharacterCore>>>>,
}

impl ArgsModalState {
    pub fn new() -> Self {
        Self {
            args: RwSignal::new(BTreeMap::new()),
            dice: RwSignal::new(BTreeMap::new()),
            valid: RwSignal::new(BTreeMap::new()),
            replacements: RwSignal::new(BTreeMap::new()),
            chain: RwSignal::new(BTreeMap::new()),
        }
    }

    /// Read AssignInputs for `key` by combining ARG signals with dice rolls.
    /// Returns empty when the key isn't registered (yet).
    pub fn inputs_for(&self, key: &FeatureKey, tracked: bool) -> Vec<AssignInputs> {
        let read_dice = |entries: &DiceSignals| -> Vec<DicePool> {
            entries
                .get(key)
                .map(|groups| {
                    groups
                        .iter()
                        .map(|dice_group| dice_group.with_value(collect_dice_pool))
                        .collect()
                })
                .unwrap_or_default()
        };
        let dice_pools: Vec<DicePool> = if tracked {
            self.dice.with(read_dice)
        } else {
            self.dice.with_untracked(read_dice)
        };
        let read_signals = |entries: &ArgsSignals| -> Vec<AssignInputs> {
            entries
                .get(key)
                .map(|groups| {
                    groups
                        .iter()
                        .enumerate()
                        .map(|(idx, sig_group)| {
                            sig_group.with_value(|signals| AssignInputs {
                                args: signals
                                    .iter()
                                    .map(|signal| {
                                        if tracked {
                                            signal.get()
                                        } else {
                                            signal.get_untracked()
                                        }
                                    })
                                    .collect(),
                                dice: dice_pools.get(idx).cloned().unwrap_or_default(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        if tracked {
            self.args.with(read_signals)
        } else {
            self.args.with_untracked(read_signals)
        }
    }

    /// Resolve a placeholder `name` → user-picked replacement.
    pub fn replacement_for(&self, name: &str, tracked: bool) -> Option<String> {
        let read = |map: &BTreeMap<String, RwSignal<Option<String>>>| {
            map.get(name).and_then(|sig| {
                if tracked {
                    sig.get()
                } else {
                    sig.get_untracked()
                }
            })
        };
        if tracked {
            self.replacements.with(read)
        } else {
            self.replacements.with_untracked(read)
        }
    }
}
/// Context provided in `CharacterLayout` so any child component can trigger
/// the args-collection modal before applying a feature.
#[derive(Clone, Copy)]
pub struct ArgsModalCtx {
    show: RwSignal<bool>,
    pending: RwSignal<Vec<PendingInputs>>,
    callback: StoredValue<Option<ArgsCallback>>,
    /// Optional seed for the cascade snapshot[0]. When `None`, the modal reads
    /// the live store — correct for level-up / user-add flows that apply on
    /// top of the existing character. Rebuild passes a fresh identity-only
    /// character so the cascade previews against a clean build-from-scratch
    /// state; the feature-edit flow passes a `build_clean(truncated_clone)`
    /// pre-edit snapshot.
    cascade_base: StoredValue<Option<Arc<CharacterCore>>>,
    /// Speculative-cascade recompute closure. When set, the modal's
    /// pick-watcher Effect runs this against a speculative character (cascade
    /// base + tentative identity picks) and updates `pending` with the
    /// returned list. `None` disables speculative recomputation — the modal
    /// renders whatever pending was passed at `open` time, unchanged.
    recompute: StoredValue<Option<RecomputePending>>,
}

impl ArgsModalCtx {
    pub fn new() -> Self {
        Self {
            show: RwSignal::new(false),
            pending: RwSignal::new(Vec::new()),
            callback: StoredValue::new(None),
            cascade_base: StoredValue::new(None),
            recompute: StoredValue::new(None),
        }
    }

    /// Show the modal for a list of features needing interaction. When the
    /// user submits, `on_complete` is called once with the collected
    /// `ApplyInputs`. `base` seeds the cascade snapshot[0]: `None` uses the
    /// live store (level-up / user-add / quick-start); `Some(character)`
    /// overrides — rebuild passes an identity-only character, edit flow
    /// passes a pre-edit snapshot. `recompute` enables speculative cascade —
    /// when an identity-slot pick changes mid-modal, the closure runs against
    /// the speculative character to recompute the pending list. `None`
    /// disables speculation (edit-feature flows where pending is fixed).
    pub fn open(
        &self,
        pending: Vec<PendingInputs>,
        base: Option<Arc<CharacterCore>>,
        recompute: Option<RecomputePending>,
        on_complete: impl FnOnce(ApplyInputs) + Send + Sync + 'static,
    ) {
        self.pending.set(pending);
        self.callback
            .update_value(|cb| *cb = Some(Box::new(on_complete)));
        self.cascade_base.set_value(base);
        self.recompute.set_value(recompute);
        self.show.set(true);
    }

    fn complete(&self, inputs: ApplyInputs) {
        self.callback.update_value(|cb| {
            if let Some(callback) = cb.take() {
                callback(inputs);
            }
        });
        self.cascade_base.set_value(None);
        self.recompute.set_value(None);
        self.show.set(false);
    }

    /// Effective cascade base for the current modal session: the value
    /// passed to `open()` if present, otherwise a fresh snapshot of the
    /// live character. Resolves the `Option` and applies the fallback in
    /// one place so callers don't repeat the pattern.
    pub fn cascade_base(&self) -> Arc<CharacterCore> {
        let store = expect_context::<Store<Character>>();
        self.cascade_base
            .with_value(|opt| opt.clone())
            .unwrap_or_else(|| Arc::new(store.read_untracked().core.clone()))
    }
}

/// One speculative cascade step on `base`: clone, apply `pending`, recompute
/// core. Always speculative — features pushed as `applied=false` so the modal
/// re-collects them as pending.
fn cascade_step(
    base: &CharacterCore,
    pending: &PendingFeature,
    registry: &RulesRegistry,
    inputs_for: &dyn Fn(&FeatureKey) -> Vec<AssignInputs>,
    replacement_for: &dyn Fn(&str) -> Option<String>,
) -> CharacterCore {
    let mut snapshot = base.clone();
    registry.with_definitions(|caches| {
        registry.with_features_index_untracked(|idx| {
            cascade(
                &mut snapshot,
                std::slice::from_ref(pending),
                idx,
                caches,
                inputs_for,
                replacement_for,
                true,
            );
        });
    });
    registry.compute_core(&mut snapshot);
    snapshot
}

/// Effective character level for a `PendingFeature`: source-embedded level
/// for Class/Subclass/User, or `base.level()` for Species/Background.
fn level_for(source: &FeatureSource, base: &CharacterCore) -> u32 {
    match source {
        FeatureSource::Class(_, level)
        | FeatureSource::Subclass(_, _, level)
        | FeatureSource::User(level) => *level,
        FeatureSource::Species(_) | FeatureSource::Background(_) => base.level().max(1),
    }
}

/// Wire one section into the per-section snapshot chain. Registers a
/// downstream signal in `state.chain[section_key]`, derives an upstream
/// from the predecessor's downstream (or cascade base for the first
/// section), and spawns an Effect-publisher that runs `cascade_step` on
/// every upstream or own-input change. Returns the upstream signal so
/// the caller's analysis Memo subscribes to the same snapshot.
fn setup_section_chain(
    section_key: FeatureKey,
    state: ArgsModalState,
) -> Signal<Arc<CharacterCore>> {
    let ctx = expect_context::<ArgsModalCtx>();
    let registry = expect_context::<RulesRegistry>();

    let downstream_signal: RwSignal<Arc<CharacterCore>> = RwSignal::new(ctx.cascade_base());
    {
        let section_key = section_key.clone();
        state.chain.update(|map| {
            map.insert(section_key, downstream_signal);
        });
    }

    let upstream_signal: Signal<Arc<CharacterCore>> = {
        let section_key = section_key.clone();
        Signal::derive(move || {
            let pending = ctx.pending.get();
            let Some(my_idx) = pending
                .iter()
                .position(|entry| entry.feature_key() == section_key)
            else {
                return ctx.cascade_base();
            };
            if my_idx == 0 {
                return ctx.cascade_base();
            }
            let prev_key = pending[my_idx - 1].feature_key();
            state
                .chain
                .with(|map| map.get(&prev_key).copied())
                .map(|prev_downstream| prev_downstream.get())
                .unwrap_or_else(|| ctx.cascade_base())
        })
    };

    {
        let section_key = section_key.clone();
        Effect::new(move |_| {
            let upstream = upstream_signal.get();
            let pending_feature = PendingFeature {
                name: section_key.name.clone(),
                source: section_key.source.clone(),
                level: level_for(&section_key.source, &upstream),
            };
            let inputs_for = |fk: &FeatureKey| -> Vec<AssignInputs> {
                if fk == &section_key {
                    state.inputs_for(fk, true)
                } else {
                    Vec::new()
                }
            };
            let replacement_for =
                |name: &str| -> Option<String> { state.replacement_for(name, true) };
            let new_downstream = cascade_step(
                &upstream,
                &pending_feature,
                &registry,
                &inputs_for,
                &replacement_for,
            );
            downstream_signal.set(Arc::new(new_downstream));
        });
    }

    on_cleanup(move || {
        state.chain.update(|map| {
            map.remove(&section_key);
        });
    });

    upstream_signal
}

/// Register `all_signals` / `all_dice` entries for a `hidden` pending feat
/// without rendering a form. The cascade's per-pending Effect reads these
/// signals to apply the feat to the next snapshot. Dice from `prefill.dice`
/// are not restored (non-interactive feats have no dice by definition).
fn register_hidden_signals(pending_inputs: &PendingInputs, all_signals: RwSignal<ArgsSignals>) {
    let key = FeatureKey::new(
        pending_inputs.feature_name.clone(),
        pending_inputs.source.clone(),
    );
    let signal_groups: Vec<StoredValue<Vec<RwSignal<i32>>>> = pending_inputs
        .prefill
        .iter()
        .map(|input| {
            let signals: Vec<RwSignal<i32>> = input
                .args
                .iter()
                .map(|value| RwSignal::new(*value))
                .collect();
            StoredValue::new(signals)
        })
        .collect();
    all_signals.update(|signals| {
        signals.insert(key, signal_groups);
    });
}

#[component]
fn ArgsFeatureInput(
    pending_inputs: PendingInputs,
    /// Upstream snapshot — the character as seen by this section's
    /// cascade. Owned by `setup_section_chain` in the parent For body;
    /// the section's analysis Memo subscribes to it for view updates.
    character: Signal<Arc<CharacterCore>>,
    state: ArgsModalState,
) -> impl IntoView {
    #[cfg(feature = "perf-marks")]
    let _mount_span = tracing::info_span!(
        "args_feature_input.mount",
        name = %pending_inputs.feature_name,
    )
    .entered();

    let registry = expect_context::<RulesRegistry>();
    let feature_name = pending_inputs.feature_name.clone();
    // Reactive label/description so switching locale updates the modal in place.
    let (feature_label, description) = registry.feature_label_desc(&feature_name);
    let has_description = {
        let description = description.clone();
        Memo::new(move |_| !description.read().is_empty())
    };
    let replace_with = pending_inputs.replace_with;
    let all_signals = state.args;
    let all_dice = state.dice;
    let all_valid = state.valid;
    let all_replacements = state.replacements;
    let replaceable = pending_inputs.is_replaceable();
    let replace_only = pending_inputs.is_replace_only();
    let source = pending_inputs.source.clone();
    let prefilled_replacement = pending_inputs.prefilled_replacement.clone();
    let replacement_prefill = pending_inputs.replacement_prefill.clone();

    let section_key = FeatureKey::new(
        pending_inputs.feature_name.clone(),
        pending_inputs.source.clone(),
    );

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

    // ARG validity for the section's own exprs and for the user-picked
    // replacement's exprs (replacement validity lifted into this scope so the
    // single section_validity memo below can AND the right set without
    // needing a synthetic key in `all_valid`).
    let own_valids: RwSignal<Vec<Memo<bool>>> = RwSignal::new(Vec::new());
    let replacement_valids: RwSignal<Vec<Memo<bool>>> = RwSignal::new(Vec::new());

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
                own_valids.update(|validations| validations.push(parts.is_valid));
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

    // Section validity branches by replacement state: when replacing, the
    // chosen feat's ARG memos (collected by `<ReplacementPicker>` into
    // `replacement_valids`) drive validity; otherwise the section's own ARG
    // memos. Single registration under section_key — `on_cleanup` below
    // removes the entry when `<For>` drops the section.
    let section_validity = Memo::new(move |_| {
        if replaceable && replacement_choice.get().is_some() {
            replacement_valids.with(|memos| memos.is_empty() || memos.iter().all(|memo| memo.get()))
        } else if replace_only {
            // Pure placeholder (Subclass marker, ASI marker without picks):
            // own ARG memos are empty so falling through would auto-validate.
            // Block submit until user picks a replacement.
            false
        } else {
            own_valids.with(|memos| memos.is_empty() || memos.iter().all(|memo| memo.get()))
        }
    });
    {
        let section_key = section_key.clone();
        all_valid.update(|map| {
            map.insert(section_key, section_validity);
        });
    }
    // Remove validity + replacement + signal entries on section unmount
    // (For drops the section when the matching pending entry disappears).
    // `all_signals` / `all_dice` reference StoredValues created in this
    // scope (line ~350) — leaving stale entries behind causes a disposed-
    // signal panic when submit walks the map. Without `all_valid` cleanup
    // dead memos accumulate and submit stays disabled forever; without
    // `all_replacements` cleanup the watcher Effect panics traversing it.
    // The section's own key plus any active replacement key (registered
    // under a different name with the same source by `ReplacementPicker`)
    // both need clearing.
    let cleanup_key = section_key.clone();
    let cleanup_feature_name = section_key.name.clone();
    let cleanup_source = section_key.source.clone();
    on_cleanup(move || {
        let replacement_key = replacement_choice
            .get_untracked()
            .map(|name| FeatureKey::new(name, cleanup_source.clone()));
        all_valid.update(|map| {
            map.remove(&cleanup_key);
        });
        all_replacements.update(|map| {
            map.remove(&cleanup_feature_name);
        });
        all_signals.update(|map| {
            map.remove(&cleanup_key);
            if let Some(key) = &replacement_key {
                map.remove(key);
            }
        });
        all_dice.update(|map| {
            map.remove(&cleanup_key);
            if let Some(key) = &replacement_key {
                map.remove(key);
            }
        });
    });

    let is_replacing = Memo::new(move |_| replacement_choice.get().is_some());

    let source_label = {
        let registry = expect_context::<RulesRegistry>();
        let i18n = expect_context::<I18n>();
        let source = source.clone();
        move || registry.source_label(&source, i18n)
    };

    view! {
        <div class="args-modal-feature">
            <h4>
                {move || feature_label.get()}
                <span class="args-modal-source">{source_label}</span>
            </h4>
            <Show when=move || has_description.get()>
                <div class="args-modal-description">
                    <Markdown text=description.clone() />
                </div>
            </Show>
            <div style:display=move || if is_replacing.get() { "none" } else { "" }>
                {expr_views}
            </div>
            {replaceable.then(|| {
                let source = source.clone();
                view! { <ReplacementPicker replace_with replacement_choice replacement_prefill character all_signals all_dice replacement_valids source replace_only /> }
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
    character: Signal<Arc<CharacterCore>>,
    all_signals: RwSignal<ArgsSignals>,
    all_dice: RwSignal<DiceSignals>,
    /// Per-section validity sink owned by the parent `<ArgsFeatureInput>`.
    /// Each chosen replacement's ARG memo pushes here; the parent's
    /// `section_validity` reads this collection when `replacement_choice`
    /// is `Some(_)`. No separate `all_valid` registration — the parent owns
    /// the section's submit-validity entry.
    replacement_valids: RwSignal<Vec<Memo<bool>>>,
    source: FeatureSource,
    replace_only: bool,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();
    let initial_replacement = replacement_choice.get_untracked();
    let replacing = RwSignal::new(replace_only || initial_replacement.is_some());
    let replacement_prefill = StoredValue::new(replacement_prefill);
    let source = StoredValue::new(source);

    let replacement_list_id = next_datalist_id();
    let options = Memo::new(move |_prev: Option<&Vec<DatalistOption>>| {
        let character = store.read();
        registry.with_features_index(|features_index| {
            // System(Class) candidates: a new class needs the full multiclass
            // gate (every existing class meets its prereq + this class's
            // prereq); an existing class always passes — level-up doesn't
            // re-check multiclass requirements.
            let class_prereqs_ok = registry.meets_class_prerequisites(&character);
            // System(Subclass) candidates: limit to subclasses of the
            // placeholder's parent class — `Subclass` placeholders are
            // attached to a specific class via `source = Class(name, lvl)`,
            // and a Cleric's picker shouldn't surface a Wizard subclass.
            let parent_class_for_subclass = source.with_value(|source| match source {
                FeatureSource::Class(name, _) | FeatureSource::Subclass(name, _, _) => {
                    Some(name.to_string())
                }
                _ => None,
            });
            features_index
                .values()
                .filter(|feat| {
                    if !replace_with.matches(feat) {
                        return false;
                    }
                    match feat.category {
                        FeatureCategory::System(IdentitySlot::Class) => {
                            let is_own = character
                                .identity
                                .classes
                                .iter()
                                .any(|class_level| class_level.class.as_str() == &*feat.name);
                            is_own || (class_prereqs_ok && feat.meets_prerequisites(&character))
                        }
                        FeatureCategory::System(IdentitySlot::Subclass) => {
                            if !feat.meets_prerequisites(&character) {
                                return false;
                            }
                            parent_class_for_subclass.as_deref().is_some_and(|parent| {
                                registry
                                    .classes()
                                    .with(parent, |class_def| {
                                        class_def.subclasses.contains_key(&*feat.name)
                                    })
                                    .unwrap_or(false)
                            })
                        }
                        _ => feat.meets_prerequisites(&character),
                    }
                })
                .map(|feat| {
                    let (label, description) = registry.feature_label_desc(&feat.name);
                    DatalistOption::with_signals(&*feat.name, label, description)
                })
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

    // Load (description, exprs) for a replacement feature name. Used by both
    // the initial-seed path (pre-filled from AI generation) and by `on_input`
    // when the user selects from the datalist.
    let load_replacement_data = move |name: &str| -> (String, Vec<Expr>) {
        let exprs = source.with_value(|source| {
            registry
                .feature_needs_args(name, Some(source))
                .map(|pending| pending.exprs)
                .unwrap_or_default()
        });
        let description = registry
            .features()
            .lookup_untracked(name, |loc| loc.description().to_string())
            .unwrap_or_default();
        (description, exprs)
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

        // Clean up stale signal/dice entries from previous replacement.
        // Same source as the section — exact key, direct remove.
        if let Some(old_name) = prev {
            let stale_key = FeatureKey::new(old_name, source.get_value());
            all_signals.update(|entries| {
                entries.remove(&stale_key);
            });
            all_dice.update(|entries| {
                entries.remove(&stale_key);
            });
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
        let label = registry
            .features()
            .lookup_untracked(name.as_str(), |loc| loc.label().to_string())
            .unwrap_or_else(|| name.clone());
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
                <SharedDatalist id=replacement_list_id.clone() options=options />
                <DatalistInput
                    value=input_value
                    placeholder=placeholder
                    list_id=replacement_list_id.clone()
                    options=options
                    on_input=on_input
                    required=true
                />
                <Show when=move || !replacement_description.with(String::is_empty)>
                    <div class="args-modal-description">
                        <Markdown text=replacement_description />
                    </div>
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
    #[cfg(feature = "perf-marks")]
    let _mount_span = tracing::info_span!("modal.open").entered();

    let ctx = expect_context::<ArgsModalCtx>();
    let title = Signal::derive(move || move_tr!("apply-features-title").get());

    // Component-scoped state — survives modal close/open cycles. The four
    // RwSignals stay separate (different shape per section type) but are
    // bundled in `ArgsModalState` so cascade closures stay short.
    let state = ArgsModalState::new();
    let all_signals = state.args;
    let all_dice = state.dice;
    let all_valid = state.valid;
    let all_replacements = state.replacements;

    // Replacement-watcher: an aggregator subscribes to BOTH the outer
    // `all_replacements` map and each inner replacement-choice signal (via
    // `sig.get()` inside the closure). On any change, build a speculative
    // character by layering each pending entry's effective feature
    // (replacement-aware) onto the cascade base, then call the modal
    // session's recompute closure. Identity writes happen as side-effects of
    // the synthesized System features' assigns.
    let registry = expect_context::<RulesRegistry>();
    // Memo (not Signal::derive) so the watcher only re-fires when the actual
    // set of chosen replacements changes — not on every all_replacements
    // mount/unmount which adds/removes None-valued entries with no semantic
    // effect.
    let replacement_choices = Memo::new(move |_| {
        all_replacements.with(|map| {
            map.iter()
                .filter_map(|(name, sig)| sig.get().map(|chosen| (name.clone(), chosen)))
                .collect::<BTreeMap<String, String>>()
        })
    });
    Effect::new(move |_| {
        if !ctx.show.get() {
            return;
        }
        let choices = replacement_choices.get();
        let recomputed = ctx.recompute.with_value(|opt| {
            let recompute = opt.as_ref()?;
            let mut speculative = (*ctx.cascade_base()).clone();
            let pending_now = ctx.pending.get_untracked();
            let inputs_for = |fk: &FeatureKey| -> Vec<AssignInputs> { state.inputs_for(fk, false) };
            let replacement_for =
                |name: &str| -> Option<String> { state.replacement_for(name, false) };
            for entry in &pending_now {
                // Unresolved replaceable placeholder — skip. Pushing the
                // placeholder into speculative.features makes subsequent
                // collect_pending_features think it's already applied,
                // creating a recompute oscillation between "placeholder
                // pending" and "placeholder absorbed".
                if entry.is_replaceable() && !choices.contains_key(&entry.feature_name) {
                    continue;
                }
                let pending_feature = PendingFeature {
                    name: entry.feature_name.clone(),
                    source: entry.source.clone(),
                    level: level_for(&entry.source, &speculative),
                };
                speculative = cascade_step(
                    &speculative,
                    &pending_feature,
                    &registry,
                    &inputs_for,
                    &replacement_for,
                );
            }
            // Identity attributes just got written by the System(_) assigns.
            // collect_pending_features (called inside `recompute`) reads the
            // class / species / background caches via untracked reads, so
            // we (a) trigger fetches for any newly-referenced definition,
            // and (b) tracked-read each cache entry so this Effect re-fires
            // when an async fetch lands and the relevant features become
            // collectable.
            registry.ensure_definitions_fetched(&speculative);
            for class_level in &speculative.identity.classes {
                if !class_level.class.is_empty() {
                    registry.classes().with(&class_level.class, |_| {});
                }
            }
            if !speculative.identity.species.is_empty() {
                registry
                    .species()
                    .with(&speculative.identity.species, |_| {});
            }
            if !speculative.identity.background.is_empty() {
                registry
                    .backgrounds()
                    .with(&speculative.identity.background, |_| {});
            }
            Some(recompute(&speculative))
        });
        // Defer overwrite until first pick AND recompute is non-empty:
        // initial pending's hidden/visible split is row-stable, and an empty
        // recompute (registry not yet loaded, etc.) shouldn't blank the modal.
        if let Some(new_pending) = recomputed
            && !choices.is_empty()
            && !new_pending.is_empty()
        {
            ctx.pending.set(new_pending);
        }
    });

    let is_valid = Memo::new(move |_| {
        all_valid.with(|map| !map.is_empty() && map.values().all(|memo| memo.get()))
    });

    let on_submit = move |event: web_sys::SubmitEvent| {
        event.prevent_default();

        let replacements: BTreeMap<String, String> = all_replacements.with_untracked(|entries| {
            entries
                .iter()
                .filter_map(|(original_name, signal)| {
                    signal
                        .get_untracked()
                        .map(|replacement| (original_name.clone(), replacement))
                })
                .collect()
        });

        let inputs_map: BTreeMap<FeatureKey, Vec<AssignInputs>> =
            all_signals.with_untracked(|sig_entries| {
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
                                        .map(|dice_sv| dice_sv.with_value(collect_dice_pool))
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

    view! {
        <Modal show=ctx.show title=title>
            <form class="args-modal-body" on:submit=on_submit>
                <For
                    each=move || ctx.pending.get()
                    key=|pending_input| pending_input.feature_key()
                    let:pending_inputs
                >
                    {
                        let section_key = pending_inputs.feature_key();
                        let upstream = setup_section_chain(section_key, state);
                        if pending_inputs.hidden {
                            register_hidden_signals(&pending_inputs, all_signals);
                            ().into_any()
                        } else {
                            view! {
                                <ArgsFeatureInput
                                    pending_inputs
                                    character=upstream
                                    state
                                />
                            }.into_any()
                        }
                    }
                </For>
                <button type="submit" class="btn-primary" disabled=move || !is_valid.get()>
                    {move_tr!("apply-features-title")}
                </button>
            </form>
        </Modal>
    }
}
