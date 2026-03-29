use leptos::prelude::*;
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;

use crate::model::{Character, CharacterStoreFields, ResistanceLevel, Translatable};

#[component]
pub fn ResistancesBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let i18n = expect_context::<I18n>();
    let resistances = store.resistances();

    move || {
        let active: Vec<_> = resistances
            .read()
            .iter()
            .filter(|(_, level)| level.is_active())
            .map(|(dt, level)| {
                let dt_name = i18n.tr(dt.tr_key());
                let level_name = i18n.tr(level.tr_key());
                match level {
                    ResistanceLevel::Resistant => dt_name,
                    _ => format!("{dt_name} ({level_name})"),
                }
            })
            .collect();

        if active.is_empty() {
            None
        } else {
            Some(view! {
                <h4 class="summary-subsection-title">{move_tr!("summary-resistances")}</h4>
                <p class="summary-resistances">{active.join(", ")}</p>
            })
        }
    }
}
