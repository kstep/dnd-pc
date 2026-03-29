use leptos::prelude::*;
use leptos_fluent::{I18n, move_tr};

use crate::{
    components::icon::Icon,
    effective::EffectiveCharacter,
    model::{DamageModifiers, DamageType, Translatable},
};

#[component]
fn DamageEntry(damage_type: DamageType, modifiers: DamageModifiers) -> impl IntoView {
    let i18n = expect_context::<I18n>();
    let icon = damage_type.icon_name();
    let label = i18n.tr(damage_type.tr_key());

    view! {
        <div class="entry-item">
            <span class="damage-dt-icon"><Icon name=icon size=14 title=label.clone() /></span>
            <div class="entry-content">
                <span class="entry-name">{label}</span>
                <span class="entry-badge damage-badge">
                    {modifiers.immune.then(|| view! {
                        <span class="damage-tag" title=move || i18n.tr("damage-immunity")>
                            <Icon name="shield-check" size=14 />
                        </span>
                    })}
                    {modifiers.resistant.then(|| view! {
                        <span class="damage-tag" title=move || i18n.tr("damage-resistance")>
                            <Icon name="shield-half" size=14 />
                        </span>
                    })}
                    {modifiers.vulnerable.then(|| view! {
                        <span class="damage-tag" title=move || i18n.tr("damage-vulnerability")>
                            <Icon name="shield-off" size=14 />
                        </span>
                    })}
                    {(modifiers.reduction > 0).then(|| view! {
                        <span class="damage-tag" title=move || i18n.tr("damage-reduction")>
                            <Icon name="shield-minus" size=14 />
                            {modifiers.reduction}
                        </span>
                    })}
                </span>
            </div>
        </div>
    }
}

#[component]
pub fn DamageModifiersBlock() -> impl IntoView {
    let effective = expect_context::<EffectiveCharacter>();

    move || {
        let entries = effective
            .damage_modifiers()
            .into_iter()
            .map(|(damage_type, modifiers)| {
                view! { <DamageEntry damage_type=damage_type modifiers=modifiers /> }
            })
            .collect_view();

        if entries.is_empty() {
            None
        } else {
            Some(view! {
                <h4 class="summary-subsection-title">{move_tr!("summary-damage-modifiers")}</h4>
                <div class="entry-list">{entries}</div>
            })
        }
    }
}
