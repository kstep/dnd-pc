use std::collections::BTreeMap;

use leptos::prelude::*;
use leptos_fluent::move_tr;

use crate::{
    components::{expr_args_input::ExprArgsInput, expr_view::ExprDetails, modal::Modal},
    rules::PendingArgs,
};

type ArgsCallback = Box<dyn Fn(BTreeMap<String, Vec<Vec<i32>>>) + Send + Sync>;
type ArgsSignals = Vec<(String, Vec<StoredValue<Vec<RwSignal<i32>>>>)>;

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
    /// submits, `on_complete` is called with a map of feature name → arg
    /// values (one inner Vec per assignment expression).
    pub fn open(
        &self,
        pending: Vec<PendingArgs>,
        on_complete: impl Fn(BTreeMap<String, Vec<Vec<i32>>>) + Send + Sync + 'static,
    ) {
        self.pending.set(pending);
        self.callback
            .with_value(|sig| sig.set(Some(StoredValue::new(Box::new(on_complete)))));
        self.show.set(true);
    }

    fn complete(&self, args_map: BTreeMap<String, Vec<Vec<i32>>>) {
        self.callback.with_value(|sig| {
            if let Some(cb) = sig.get_untracked() {
                cb.with_value(|f| f(args_map));
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
) -> impl IntoView {
    let feature_name = pa.feature_name.clone();
    let description = pa.feature_description.clone();
    let has_description = !description.is_empty();

    // Collect signal groups for all exprs of this feature
    let signal_groups: StoredValue<Vec<StoredValue<Vec<RwSignal<i32>>>>> =
        StoredValue::new(Vec::new());
    let name_for_signals = feature_name.clone();

    let expr_views = pa
        .exprs
        .into_iter()
        .map(|expr| {
            let on_ready = move |parts: crate::components::expr_args_input::ExprArgsInputParts| {
                signal_groups.update_value(|groups| {
                    groups.push(StoredValue::new(parts.rw_signals));
                });
                all_valid.update(|v| v.push(parts.is_valid));
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

    view! {
        <div class="args-modal-feature">
            <h4>{pa.feature_label.clone()}</h4>
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

                let all_signals: RwSignal<ArgsSignals> =
                    RwSignal::new(Vec::new());
                let all_valid: RwSignal<Vec<Memo<bool>>> = RwSignal::new(Vec::new());

                let feature_views = pending
                    .into_iter()
                    .map(|pa| {
                        view! { <ArgsFeatureInput pa all_signals all_valid /> }
                    })
                    .collect_view();

                let is_valid = Memo::new(move |_| {
                    all_valid.with(|v| !v.is_empty() && v.iter().all(|m| m.get()))
                });

                let on_submit = move |ev: web_sys::SubmitEvent| {
                    ev.prevent_default();
                    let mut map: BTreeMap<String, Vec<Vec<i32>>> = BTreeMap::new();
                    all_signals.with_untracked(|entries| {
                        for (name, groups) in entries {
                            let feature_args: Vec<Vec<i32>> = groups
                                .iter()
                                .map(|sigs| {
                                    sigs.with_value(|sigs| {
                                        sigs.iter().map(|s| s.get_untracked()).collect()
                                    })
                                })
                                .collect();
                            map.insert(name.clone(), feature_args);
                        }
                    });
                    ctx.complete(map);
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
