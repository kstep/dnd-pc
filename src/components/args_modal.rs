use std::collections::BTreeMap;

use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{
        datalist_input::DatalistInput, expr_args_input::ExprArgsInput, expr_view::ExprDetails,
        modal::Modal,
    },
    model::Character,
    rules::{PendingArgs, RulesRegistry},
};

type ArgsCallback = Box<dyn Fn(ArgsModalResult) + Send + Sync>;
type ArgsSignals = Vec<(String, Vec<StoredValue<Vec<RwSignal<i32>>>>)>;

/// Result returned by the ArgsModal on submit.
pub struct ArgsModalResult {
    /// Feature name → arg values for each assignment expression.
    pub args: BTreeMap<String, Vec<Vec<i32>>>,
    /// Original feature name → replacement feature name.
    pub replacements: BTreeMap<String, String>,
}

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

    /// Show the modal for a list of features needing args. When the user
    /// submits, `on_complete` is called with args and replacement maps.
    pub fn open(
        &self,
        pending: Vec<PendingArgs>,
        on_complete: impl Fn(ArgsModalResult) + Send + Sync + 'static,
    ) {
        self.pending.set(pending);
        self.callback
            .with_value(|sig| sig.set(Some(StoredValue::new(Box::new(on_complete)))));
        self.show.set(true);
    }

    fn complete(&self, result: ArgsModalResult) {
        self.callback.with_value(|sig| {
            if let Some(cb) = sig.get_untracked() {
                cb.with_value(|f| f(result));
            }
            sig.set(None);
        });
        self.show.set(false);
    }
}

#[component]
fn ArgsFeatureInput(
    pa: PendingArgs,
    all_signals: RwSignal<ArgsSignals>,
    all_valid: RwSignal<Vec<Memo<bool>>>,
    all_replacements: RwSignal<BTreeMap<String, RwSignal<Option<String>>>>,
) -> impl IntoView {
    let feature_name = pa.feature_name.clone();
    let description = pa.feature_description.clone();
    let has_description = !description.is_empty();
    let replaceable = pa.replaceable;
    let has_exprs = !pa.exprs.is_empty();

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
    let name_for_signals = feature_name.clone();

    // For replaceable features, collect expr validity locally so we can
    // bypass it when the user picks a replacement.
    let expr_valids: RwSignal<Vec<Memo<bool>>> = RwSignal::new(Vec::new());

    let expr_views = pa
        .exprs
        .into_iter()
        .map(|expr| {
            let on_ready = move |parts: crate::components::expr_args_input::ExprArgsInputParts| {
                signal_groups.update_value(|groups| {
                    groups.push(StoredValue::new(parts.rw_signals));
                });
                if replaceable {
                    expr_valids.update(|v| v.push(parts.is_valid));
                } else {
                    all_valid.update(|v| v.push(parts.is_valid));
                }
            };
            view! {
                <ExprDetails expr=expr.clone() />
                <ExprArgsInput expr on_ready />
            }
        })
        .collect_view();

    // Register all signal groups for this feature after building
    all_signals.update(|v| {
        signal_groups.with_value(|groups| {
            v.push((name_for_signals.clone(), groups.clone()));
        });
    });

    // For replaceable features, push a single combined validity memo:
    // valid if (replacing with a chosen feat) OR (not replacing AND all
    // ARG expr memos pass). For replaceable-only features (no exprs),
    // expr_valids is empty so the fallback is always valid.
    if replaceable {
        all_valid.update(|v| {
            v.push(Memo::new(move |_| {
                if replacement_choice.get().is_some() {
                    return true;
                }
                expr_valids.with(|fv| fv.is_empty() || fv.iter().all(|m| m.get()))
            }));
        });
    }

    let is_replacing = Memo::new(move |_| replacement_choice.get().is_some());

    view! {
        <div class="args-modal-feature">
            <h4>{pa.feature_label.clone()}</h4>
            <Show when=move || has_description>
                <p class="args-modal-description">{description.clone()}</p>
            </Show>
            <div style:display=move || if is_replacing.get() { "none" } else { "" }>
                {expr_views}
            </div>
            {replaceable.then(|| {
                view! { <ReplacementPicker feature_name=feature_name.clone() replacement_choice /> }
            })}
        </div>
    }
}

#[component]
fn ReplacementPicker(
    #[allow(unused)] feature_name: String,
    replacement_choice: RwSignal<Option<String>>,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();
    let replacing = RwSignal::new(false);

    let options = Signal::derive(move || {
        let character = store.read();
        registry.with_features_index(|features_index| {
            features_index
                .values()
                .filter(|feat| feat.selectable && feat.meets_prerequisites(&character))
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

    let on_input = move |_text: String, resolved: Option<String>| {
        replacement_choice.set(resolved.clone());
        if let Some(name) = resolved {
            input_value.set(name);
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

                let all_signals: RwSignal<ArgsSignals> =
                    RwSignal::new(Vec::new());
                let all_valid: RwSignal<Vec<Memo<bool>>> = RwSignal::new(Vec::new());
                let all_replacements: RwSignal<BTreeMap<String, RwSignal<Option<String>>>> =
                    RwSignal::new(BTreeMap::new());

                let feature_views = pending
                    .into_iter()
                    .map(|pa| {
                        view! { <ArgsFeatureInput pa all_signals all_valid all_replacements /> }
                    })
                    .collect_view();

                let is_valid = Memo::new(move |_| {
                    all_valid.with(|v| !v.is_empty() && v.iter().all(|m| m.get()))
                });

                let on_submit = move |ev: web_sys::SubmitEvent| {
                    ev.prevent_default();

                    // Collect replacement decisions
                    let mut replacements: BTreeMap<String, String> = BTreeMap::new();
                    all_replacements.with_untracked(|entries| {
                        for (original_name, signal) in entries {
                            if let Some(replacement_name) = signal.get_untracked() {
                                replacements
                                    .insert(original_name.clone(), replacement_name);
                            }
                        }
                    });

                    // Collect args (skip features that were replaced)
                    let mut args: BTreeMap<String, Vec<Vec<i32>>> = BTreeMap::new();
                    all_signals.with_untracked(|entries| {
                        for (name, groups) in entries {
                            if replacements.contains_key(name) {
                                continue;
                            }
                            let feature_args: Vec<Vec<i32>> = groups
                                .iter()
                                .map(|sigs| {
                                    sigs.with_value(|sigs| {
                                        sigs.iter().map(|s| s.get_untracked()).collect()
                                    })
                                })
                                .collect();
                            args.insert(name.clone(), feature_args);
                        }
                    });

                    ctx.complete(ArgsModalResult { args, replacements });
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
