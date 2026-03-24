use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::components::A;

use super::{ReferenceFeaturesView, collect_feature_views};
use crate::{
    BASE_URL,
    components::icon::Icon,
    hooks::{use_debounce, use_query_signal},
    rules::RulesRegistry,
};

#[component]
pub fn FeatureReference() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let i18n = expect_context::<leptos_fluent::I18n>();
    let (search, set_search) = use_query_signal::<String>("q");
    let debounce = use_debounce(300);

    let features_view = move || {
        let query = search.read().as_deref().unwrap_or_default().to_lowercase();
        registry.with_features_index(|idx| {
            let filtered = idx.values().filter(|feat| {
                if query.is_empty() {
                    feat.selectable
                } else {
                    feat.label().to_lowercase().contains(&query)
                }
            });
            let features = collect_feature_views(filtered);
            if features.is_empty() {
                None
            } else {
                Some(view! { <ReferenceFeaturesView features /> })
            }
        })
    };

    view! {
        <Title text=Signal::derive(move || i18n.tr("ref-features")) />
        <div class="reference-page">
            <div class="reference-feature-page">
                <div class="reference-feature-header">
                    <A href=format!("{BASE_URL}/") attr:class="reference-home-link">
                        <Icon name="arrow-left" size=16 />
                    </A>
                    <input
                        type="search"
                        class="reference-search"
                        placeholder=move_tr!("ref-search-feature")
                        prop:value=move || search.get().unwrap_or_default()
                        on:input=move |event| {
                            let value = event_target_value(&event);
                            debounce(Box::new(move || {
                                set_search.set(if value.is_empty() { None } else { Some(value) });
                            }));
                        }
                    />
                </div>
                <main class="reference-main" style="margin-left: var(--size-8);">
                    {features_view}
                </main>
            </div>
        </div>
    }
}
