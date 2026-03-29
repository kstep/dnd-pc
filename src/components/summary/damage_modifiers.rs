use leptos::prelude::*;
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;

use crate::{
    components::icon::Icon,
    model::{Character, CharacterStoreFields, Translatable},
};

#[component]
pub fn DamageModifiersBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let i18n = expect_context::<I18n>();
    let damage_modifiers = store.damage_modifiers();

    move || {
        let active: Vec<_> = damage_modifiers
            .read()
            .iter()
            .filter(|(_, mods)| mods.is_active())
            .map(|(dt, mods)| (*dt, *mods))
            .collect();

        if active.is_empty() {
            None
        } else {
            let last = active.len() - 1;
            let entries = active
                .into_iter()
                .enumerate()
                .map(|(idx, (dt, mods))| {
                    let icon = dt.icon_name();
                    let label = i18n.tr(dt.tr_key());
                    let separator = if idx < last { ", " } else { "" };
                    view! {
                        <span class="damage-entry">
                            <Icon name=icon size=14 />
                            {label}
                            {mods.immune.then(|| view! {
                                <span class="damage-tag damage-immunity" title=move || i18n.tr("damage-immunity")>
                                    <Icon name="shield-check" size=12 />
                                </span>
                            })}
                            {mods.resistant.then(|| view! {
                                <span class="damage-tag damage-resistance" title=move || i18n.tr("damage-resistance")>
                                    <Icon name="shield-half" size=12 />
                                </span>
                            })}
                            {mods.vulnerable.then(|| view! {
                                <span class="damage-tag damage-vulnerability" title=move || i18n.tr("damage-vulnerability")>
                                    <Icon name="shield-off" size=12 />
                                </span>
                            })}
                            {(mods.reduction > 0).then(|| view! {
                                <span class="damage-tag damage-reduction" title=move || i18n.tr("damage-reduction")>
                                    <Icon name="shield-minus" size=12 />
                                    {mods.reduction}
                                </span>
                            })}
                            {separator}
                        </span>
                    }
                })
                .collect_view();

            Some(view! {
                <h4 class="summary-subsection-title">{move_tr!("summary-damage-modifiers")}</h4>
                <div class="summary-damage-modifiers">{entries}</div>
            })
        }
    }
}
