use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{apply::replay_with_modal, hint_banner::HintBanner},
    hooks::use_hash_href,
    model::{Character, Feature, FeatureValue},
    rules::RulesRegistry,
};

fn has_empty_choice(feature: &Feature, character: &Character) -> bool {
    character
        .features
        .data()
        .get(&feature.name)
        .is_some_and(|feature_data| {
            feature_data.fields.iter().any(|field| {
                matches!(
                    &field.value,
                    FeatureValue::Choice { options }
                        if options.iter().any(|opt| opt.label().is_empty())
                )
            })
        })
}

fn feature_link_list<F>(store: Store<Character>, predicate: F) -> impl IntoView
where
    F: Fn(&Feature, &Character) -> bool + Copy + Send + Sync + 'static,
{
    let hash_href = use_hash_href();
    view! {
        <span class="hint-banner-list">
            {move || {
                let character = store.read();
                character
                    .features
                    .iter()
                    .filter(|feature| predicate(feature, &character))
                    .enumerate()
                    .map(|(idx, feature)| {
                        view! {
                            {(idx > 0).then_some(", ")}
                            <a rel="external" href=hash_href(&feature.dom_id())>
                                {feature.label().to_string()}
                            </a>
                        }
                    })
                    .collect_view()
            }}
        </span>
    }
}

#[component]
pub fn BuildReplayHint() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();
    let visible =
        Signal::derive(move || store.read().features.iter().any(|feature| !feature.applied));
    view! {
        <HintBanner
            icon="wand-sparkles"
            class="hint-banner-wide"
            visible=visible
            action_label=move_tr!("btn-replay-all")
            on_action=Callback::new(move |()| replay_with_modal(store, registry))
        >
            <p class="hint-banner-text">{move_tr!("build-replay-hint-title")}</p>
            {feature_link_list(store, |feature, _| !feature.applied)}
        </HintBanner>
    }
}

#[component]
pub fn BuildChoiceFillHint() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let visible = Signal::derive(move || {
        let character = store.read();
        character
            .features
            .iter()
            .any(|feature| has_empty_choice(feature, &character))
    });
    view! {
        <HintBanner icon="list-checks" class="hint-banner-wide" visible=visible>
            <p class="hint-banner-text">{move_tr!("build-choice-hint-title")}</p>
            {feature_link_list(store, has_empty_choice)}
        </HintBanner>
    }
}
