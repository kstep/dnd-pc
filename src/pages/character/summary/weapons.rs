use leptos::{either::Either, prelude::*};
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;

use crate::{
    components::summary_list::{SummaryList, SummaryListItem},
    model::{Character, CharacterStoreFields, EquipmentStoreFields, Translatable},
};

#[component]
pub fn WeaponsBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let i18n = expect_context::<I18n>();
    let items = store
        .equipment()
        .weapons()
        .read()
        .iter()
        .filter(|w| !w.name.is_empty())
        .map(|w| {
            let dmg_type = w.damage_type.map(|dt| i18n.tr(dt.tr_key()));

            let name_atk = if w.attack_bonus != 0 {
                format!("{} {:+}", w.name, w.attack_bonus)
            } else {
                w.name.clone()
            };

            let damage_info = if let Some(dtype) = dmg_type {
                format!("{dtype} {}", w.damage)
            } else {
                w.damage.clone()
            };

            SummaryListItem {
                name: name_atk,
                description: String::new(),
                badge: Some(
                    view! {
                        <span class="summary-list-badge">{damage_info}</span>
                    }
                    .into_any(),
                ),
            }
        })
        .collect::<Vec<_>>();

    if items.is_empty() {
        Either::Left(view! {
            <p class="summary-empty">{move_tr!("summary-no-weapons")}</p>
        })
    } else {
        Either::Right(view! {
            <div class="summary-subsection">
                <h4 class="summary-subsection-title">{move_tr!("weapons")}</h4>
                <SummaryList items=items />
            </div>
        })
    }
}
