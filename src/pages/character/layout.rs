use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::{components::A, hooks::use_params, nested_router::Outlet, params::Params};
use reactive_stores::Store;
use uuid::Uuid;

use crate::{
    BASE_URL,
    effective::EffectiveCharacter,
    model::{Character, CharacterIdentityStoreFields, CharacterStoreFields},
    rules::RulesRegistry,
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
                    <A href=format!("{BASE_URL}/")>{move_tr!("back-to-list")}</A>
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

    // Load and provide active effects (separate from character, not synced).
    let char_id = store.read_untracked().id;
    let mut initial_effects = storage::load_effects(&char_id);
    initial_effects.recompute(&store.read_untracked());
    let effects = RwSignal::new(initial_effects);
    provide_context(EffectiveCharacter::new(store, effects));

    // Recompute effects and propagate consumable overrides (Hp, TempHp)
    // once on first appearance (memoized to avoid resetting user edits).
    Effect::new(move || {
        let needs_propagation = store
            .try_with(|character| {
                effects.track();
                effects
                    .try_update_untracked(|eff| eff.recompute(character))
                    .unwrap_or(false)
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

    // Auto-save + cloud sync pull (touch gated on initial sync).
    storage::setup_auto_save(store);

    // Trigger definition fetches when the index arrives or character changes.
    // This is cheap — just kicks off async fetches, no store mutation.
    let registry = expect_context::<RulesRegistry>();
    Effect::new(move || {
        store.with(|c| {
            registry.ensure_definitions_fetched(c);
        });
    });

    // Fill labels from cached definitions. Separate effect so index changes
    // (which only trigger fetches) don't cause expensive store updates.
    Effect::new(move || {
        store.update(|c| {
            registry.fill_from_registry(c);
        });
    });

    // No separate locale-change effect needed — fill_from_registry always
    // overwrites labels/descriptions, so when caches update with new locale
    // data, fill re-runs and applies the new labels directly.

    let name = Memo::new(move |_| store.identity().name().get());
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
        <div class="character-sheet">
            <Outlet />
        </div>
    }
}
