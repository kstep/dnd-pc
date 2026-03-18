use leptos::{either::Either, prelude::*};
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;

use crate::{
    components::summary_list::{SummaryList, SummaryListItem},
    effective::EffectiveCharacter,
    model::{Character, CharacterStoreFields, EquipmentStoreFields, Translatable},
};

#[component]
pub fn WeaponsBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let eff = expect_context::<EffectiveCharacter>();
    let i18n = expect_context::<I18n>();
    let weapons = store.equipment().weapons();

    move || {
        let global_atk = eff.attack_bonus();
        let items = weapons
            .read()
            .iter()
            .filter(|w| !w.name.is_empty())
            .map(|w| {
                let dmg_type = w.damage_type.map(|dt| i18n.tr(dt.tr_key()));

                let total_atk = w.attack_bonus + global_atk;
                let name_atk = if total_atk != 0 {
                    format!("{} {:+}", w.name, total_atk)
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
                            <span class="entry-badge">{damage_info}</span>
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
}
