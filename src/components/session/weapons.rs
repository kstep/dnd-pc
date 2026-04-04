use std::collections::BTreeMap;

use leptos::{either::Either, prelude::*};
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;

use crate::{
    components::{
        cast_button::CastButton,
        effects_calc_modal::{EffectsCalcInfo, EffectsCalcModal},
        icon::Icon,
        session_list::{SessionList, SessionListItem},
    },
    effective::EffectiveCharacter,
    model::{
        Character, CharacterStoreFields, EffectDefinition, EquipmentStoreFields, Translatable,
    },
};

#[component]
pub fn WeaponsBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let eff = expect_context::<EffectiveCharacter>();
    let i18n = expect_context::<I18n>();
    let weapons = store.equipment().weapons();

    let show_calc = RwSignal::new(false);
    let calc_info = StoredValue::new(None::<EffectsCalcInfo>);

    let content = move || {
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

                let active_effects: Vec<_> =
                    w.effects.iter().filter(|e| !e.expr.is_empty()).collect();
                let has_effects = !active_effects.is_empty();

                // First effect as inline badge, "…" if more
                let first_badge = active_effects.first().map(|effect| {
                    let icon = effect
                        .damage_type
                        .map(|dt| view! { <Icon name=dt.icon_name() size=14 /> });
                    let title = if effect.name.is_empty() {
                        None
                    } else {
                        Some(effect.name.clone())
                    };
                    let expr = effect.expr.to_string();
                    view! { <span class="entry-badge" title=title>{icon}" "{expr}</span> }
                });
                let more = (active_effects.len() > 1)
                    .then(|| view! { <span class="entry-badge">"\u{2026}"</span> });

                // Full list for expandable description
                let description = if active_effects.len() > 1 {
                    active_effects
                        .iter()
                        .map(|effect| {
                            let dt = effect
                                .damage_type
                                .map(|dt| i18n.tr(dt.tr_key()))
                                .unwrap_or_default();
                            let name = &effect.name;
                            let expr = &effect.expr;
                            if name.is_empty() {
                                format!("{dt} {expr}")
                            } else {
                                format!("{dt} {expr} ({name})")
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    String::new()
                };

                let cast_button = has_effects.then(|| {
                    let effects: Vec<EffectDefinition> =
                        w.effects.iter().map(EffectDefinition::from).collect();
                    let title = name_atk.clone();
                    view! {
                        <CastButton on_cast=Callback::new(move |_| {
                            calc_info.set_value(Some(EffectsCalcInfo {
                                title: title.clone(),
                                effects: effects.clone(),
                                extra_vars: BTreeMap::new(),
                                spell_name: String::new(),
                                feature_name: String::new(),
                            }));
                            show_calc.set(true);
                        }) />
                    }
                });

                SessionListItem {
                    name: name_atk,
                    description,
                    badge: if first_badge.is_none() && cast_button.is_none() {
                        None
                    } else {
                        Some(view! { <>{first_badge}{more}{cast_button}</> }.into_any())
                    },
                }
            })
            .collect::<Vec<_>>();

        let attack_count = eff.attack_count();

        if items.is_empty() {
            Either::Left(view! {
                <p class="session-empty">{move_tr!("session-no-weapons")}</p>
            })
        } else {
            Either::Right(view! {
                <div class="session-subsection">
                    <h4 class="session-subsection-title">
                        {move_tr!("weapons")}
                        {(attack_count > 1).then(|| view! {
                            <span class="entry-badge">{move_tr!("attack-count")} ": " {attack_count}</span>
                        })}
                    </h4>
                    <SessionList items=items />
                </div>
            })
        }
    };

    view! {
        {content}
        <EffectsCalcModal show=show_calc info=calc_info />
    }
}
