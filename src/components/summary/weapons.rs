use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{
        icon::Icon,
        summary_list::{SummaryList, SummaryListItem},
    },
    effective::EffectiveCharacter,
    model::{Character, CharacterStoreFields, EquipmentStoreFields},
};

#[component]
pub fn WeaponsBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let eff = expect_context::<EffectiveCharacter>();
    let weapons = store.equipment().weapons();

    move || {
        let global_atk = eff.attack_bonus();
        let items = weapons
            .read()
            .iter()
            .filter(|w| !w.name.is_empty())
            .map(|w| {
                let total_atk = w.attack_bonus + global_atk;
                let name_atk = if total_atk != 0 {
                    format!("{} {:+}", w.name, total_atk)
                } else {
                    w.name.clone()
                };

                let badges: Vec<_> = w
                    .effects
                    .iter()
                    .map(|effect| {
                        let icon = effect
                            .damage_type
                            .map(|dt| view! { <Icon name=dt.icon_name() size=14 /> });
                        let label = if effect.name.is_empty() {
                            effect.expr.to_string()
                        } else {
                            format!("{}: {}", effect.name, effect.expr)
                        };
                        view! { <span class="entry-badge">{icon}" "{label}</span> }.into_any()
                    })
                    .collect();

                SummaryListItem {
                    name: name_atk,
                    description: String::new(),
                    badge: if badges.is_empty() {
                        None
                    } else {
                        Some(view! { <>{badges}</> }.into_any())
                    },
                }
            })
            .collect::<Vec<_>>();

        let attack_count = eff.attack_count();

        if items.is_empty() {
            Either::Left(view! {
                <p class="summary-empty">{move_tr!("summary-no-weapons")}</p>
            })
        } else {
            Either::Right(view! {
                <div class="summary-subsection">
                    <h4 class="summary-subsection-title">
                        {move_tr!("weapons")}
                        {(attack_count > 1).then(|| view! {
                            <span class="entry-badge">{move_tr!("attack-count")} ": " {attack_count}</span>
                        })}
                    </h4>
                    <SummaryList items=items />
                </div>
            })
        }
    }
}
