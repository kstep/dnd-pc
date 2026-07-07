use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::{hooks::use_params, nested_router::Outlet, params::Params};
use reactive_stores::Store;
use uuid::Uuid;

use crate::{
    components::{
        args_modal::{ArgsModal, ArgsModalCtx},
        cloud_sign_in_hint::CloudSignInHint,
        navbar::ActiveCharacterId,
        ref_link::Ref,
    },
    effective::EffectiveCharacter,
    model::{Character, CharacterStoreFields, PersonalityStoreFields},
    rules::{ActivePackages, RulesRegistry},
    storage,
};

#[derive(Params, Clone, Debug, PartialEq, Eq)]
struct CharacterParams {
    id: Uuid,
}

#[component]
pub fn CharacterLayout() -> impl IntoView {
    let params = use_params::<CharacterParams>();

    let id = Memo::new(move |_| params.get().ok().map(|params| params.id));

    move || {
        if let Some(char_data) = id.get().and_then(|id| storage::load_character(&id)) {
            Either::Left(view! { <CharacterInner char_data /> })
        } else {
            Either::Right(view! {
                <div class="not-found">
                    <h1>{move_tr!("character-not-found")}</h1>
                    <CloudSignInHint message=move_tr!("hint-character-not-found") />
                    <Ref href="/">{move_tr!("back-to-list")}</Ref>
                </div>
            })
        }
    }
}

#[component]
fn CharacterInner(char_data: Character) -> impl IntoView {
    let store = Store::new(char_data);

    // Provide context first so child components can access the store.
    provide_context(store);

    // Set active character ID for the navbar.
    let active_id = expect_context::<ActiveCharacterId>().0;
    active_id.set(Some(store.read_untracked().id));
    on_cleanup(move || active_id.set(None));

    // Provide args-modal context for features that need user input during apply.
    let args_modal_ctx = ArgsModalCtx::new();
    provide_context(args_modal_ctx);

    // Load and provide active effects (separate from character, not synced).
    let char_id = store.read_untracked().id;
    let mut initial_effects = storage::load_effects(&char_id);
    let registry = expect_context::<RulesRegistry>();
    registry.with_features_index_untracked(|feat_index| {
        initial_effects.recompute(&store.read_untracked(), feat_index);
    });
    let effects = RwSignal::new(initial_effects);
    provide_context(EffectiveCharacter::new(store, effects));

    // Load and provide avatar (separate from character, cloud-synced separately).
    let initial_avatar = storage::load_avatar(&char_id);
    let avatar = RwSignal::new(initial_avatar);
    provide_context(avatar);

    // Recompute effects and propagate consumable overrides (Hp, TempHp)
    // once on first appearance (memoized to avoid resetting user edits).
    Effect::new(move || {
        let needs_propagation = store
            .try_with(|character| {
                effects.track();
                registry.with_features_index_untracked(|feat_index| {
                    effects
                        .try_update_untracked(|eff| eff.recompute(character, feat_index))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        if needs_propagation {
            store.update(|c| {
                effects.try_update_untracked(|eff| eff.propagate(c));
            });
        }
    });

    // Auto-save effects when they change.
    Effect::new(move || {
        let eff = effects.read();
        storage::save_effects(&char_id, &eff);
    });

    // Auto-save avatar + cloud-pull with echo suppression and timestamp guard.
    storage::setup_avatar_auto_save(char_id, avatar);

    // Auto-save + cloud sync pull (touch gated on initial sync).
    storage::setup_auto_save(store);

    // Follow the character: its package set becomes the active one. While
    // the triggered refetch is in flight the registry serves the previous
    // merged set — same accepted staleness as a locale switch; the Fill
    // effect tracks the indexes and re-runs when they land.
    let active_packages = expect_context::<ActivePackages>();
    Effect::new(move || {
        let packages = store.packages().get();
        if !packages.is_empty()
            && active_packages
                .0
                .with_untracked(|active| active != &packages)
        {
            active_packages.0.set(packages);
        }
    });

    // Trigger spell-list fetches when the indexes arrive or character
    // changes. This is cheap — just kicks off async fetches, no store
    // mutation.
    Effect::new(move || {
        store.with(|c| {
            registry.ensure_definitions_fetched(c);
        });
    });

    // Fill labels from cached definitions. Effect deps are the locale
    // resources (subscribed via `track()` calls inside `fill_from_registry`);
    // `store.update` notifies views so they re-render with the just-synced
    // labels. We do NOT track the store here — that would loop on our own
    // write. Character-mutating callers (apply pipeline, rest, rebuild)
    // refill labels themselves inside their `store.update` block.
    Effect::new(move || {
        store.update(|c| {
            registry.fill_from_registry(c);
        });
    });

    let name = Memo::new(move |_| store.personality().name().get());
    let class_summary = Memo::new(move |_| store.read().class_summary());
    let title = move || {
        let name = name.read();
        let summary = class_summary.read();
        match (name.is_empty(), summary.is_empty()) {
            (true, true) => "D&D PC".to_string(),
            (true, false) => summary.to_string(),
            (false, true) => name.to_string(),
            (false, false) => format!("{name} — {summary}"),
        }
    };

    view! {
        <Title text=title />
        <div class="character-editor">
            <Outlet />
            <ArgsModal />
        </div>
    }
}
