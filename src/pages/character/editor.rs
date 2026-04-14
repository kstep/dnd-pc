use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_router::{
    hooks::{use_location, use_params},
    nested_router::Outlet,
    params::Params,
};
use reactive_stores::Store;
use uuid::Uuid;

use crate::{
    BASE_URL,
    components::{
        character_header::CharacterHeader,
        tab_nav::{TabItem, TabNav},
    },
    model::{Character, FeatureValue},
    storage,
};

const KNOWN_TABS: &[&str] = &["stats", "build", "magic", "inventory", "backstory"];

#[derive(Params, Clone, Debug, PartialEq, Eq)]
struct EditorParams {
    id: Uuid,
}

#[component]
pub fn CharacterEditor() -> impl IntoView {
    let params = use_params::<EditorParams>();
    let base = Signal::derive(move || {
        params
            .get()
            .ok()
            .map(|p| format!("{BASE_URL}/c/{}", p.id))
            .unwrap_or_default()
    });

    let store = expect_context::<Store<Character>>();
    let has_magic = Signal::derive(move || !store.read().spell_slots.is_empty());
    let magic_has_pending = Signal::derive(move || {
        store.read().feature_data.values().any(|fd| {
            fd.spells
                .as_ref()
                .is_some_and(|sd| sd.spells.iter().any(|s| s.label().is_empty()))
        })
    });
    let build_has_pending = Signal::derive(move || {
        let character = store.read();
        character.features.iter().any(|f| !f.applied)
            || character.feature_data.values().any(|fd| {
                fd.fields.iter().any(|f| {
                matches!(
                    &f.value,
                    FeatureValue::Choice { options } if options.iter().any(|o| o.name.is_empty())
                )
            })
            })
    });

    let items = vec![
        TabItem::new("stats", move_tr!("tab-stats"), "scroll-text"),
        TabItem::new("build", move_tr!("tab-build"), "settings").marked_when(build_has_pending),
        TabItem::new("magic", move_tr!("tab-magic"), "wand")
            .visible_when(has_magic)
            .marked_when(magic_has_pending),
        TabItem::new("inventory", move_tr!("tab-inventory"), "backpack"),
        TabItem::new("backstory", move_tr!("tab-backstory"), "book-open"),
    ];

    let location = use_location();
    Effect::new(move |_| {
        let path = location.pathname.read();
        if let Some(tab) = path.rsplit('/').next()
            && KNOWN_TABS.contains(&tab)
        {
            storage::save_last_editor_tab(tab);
        }
    });

    view! {
        <CharacterHeader />
        <TabNav base=base items=items />
        <Outlet />
    }
}
