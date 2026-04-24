use std::collections::HashMap;

use leptos::prelude::*;
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;

use crate::{
    components::{
        apply::{apply_pending, rebuild, replay_with_modal},
        hint_banner::HintBanner,
    },
    hooks::use_hash_href,
    model::{Character, Feature, FeatureValue, RebuildReason},
    rules::RulesRegistry,
};

impl RebuildReason {
    fn label(&self, i18n: &I18n) -> String {
        match self {
            Self::SpeciesChanged => i18n.tr("rebuild-reason-species").into_owned(),
            Self::BackgroundChanged => i18n.tr("rebuild-reason-background").into_owned(),
            Self::ClassRemoved(class) => {
                let mut args = HashMap::new();
                args.insert("class".into(), class.clone().into());
                i18n.tr_with_args("rebuild-reason-class-removed", &args)
            }
            Self::LevelLowered {
                class,
                applied,
                current,
            } => {
                let mut args = HashMap::new();
                args.insert("class".into(), class.clone().into());
                args.insert("applied".into(), (*applied).into());
                args.insert("current".into(), (*current).into());
                i18n.tr_with_args("rebuild-reason-level-lowered", &args)
            }
        }
    }
}

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
            action_label=move_tr!("replay")
            on_action=Callback::new(move |()| replay_with_modal(store, registry))
        >
            <p class="hint-banner-text">{move_tr!("build-replay-hint-title")}</p>
            {feature_link_list(store, |feature, _| !feature.applied)}
        </HintBanner>
    }
}

#[component]
pub fn BuildNeedsRebuildHint() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();
    let i18n = expect_context::<I18n>();
    let reasons = Signal::derive(move || store.read().rebuild_reasons());
    let visible = Signal::derive(move || !reasons.read().is_empty());
    view! {
        <HintBanner
            icon="wrench"
            class="hint-banner-wide"
            visible=visible
            action_label=move_tr!("rebuild")
            on_action=Callback::new(move |()| rebuild(store, registry))
        >
            <p class="hint-banner-text">{move_tr!("build-needs-rebuild-title")}</p>
            <ul class="hint-banner-reasons">
                {move || {
                    reasons
                        .get()
                        .into_iter()
                        .map(|reason| view! {
                            <li>{reason.label(&i18n)}</li>
                        })
                        .collect_view()
                }}
            </ul>
        </HintBanner>
    }
}

#[component]
pub fn BuildPendingApplyHint() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();
    let visible = Signal::derive(move || {
        let character = store.read();
        !character.needs_rebuild() && character.has_pending_apply()
    });
    view! {
        <HintBanner
            icon="arrow-up"
            class="hint-banner-wide"
            visible=visible
            action_label=move_tr!("apply")
            on_action=Callback::new(move |()| apply_pending(store, registry))
        >
            <p class="hint-banner-text">{move_tr!("build-pending-apply-title")}</p>
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
