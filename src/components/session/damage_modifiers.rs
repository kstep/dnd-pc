use leptos::prelude::*;
use leptos_fluent::{I18n, move_tr};

use crate::{
    components::{entry_name::EntryName, icon::Icon},
    effective::EffectiveCharacter,
    model::{DamageModifier, DamageType, Translatable},
};

#[component]
fn DamageEntry(damage_type: DamageType, modifiers: DamageModifier) -> impl IntoView {
    let i18n = expect_context::<I18n>();
    let icon = damage_type.icon_name();
    let tr_key = damage_type.tr_key();
    let title = untrack(|| i18n.tr(tr_key).into_owned());
    let label = Signal::derive(move || i18n.tr(tr_key).into_owned());

    view! {
        <div class="entry-item">
            <span class="damage-dt-icon">
                <Icon name=icon title=title />
            </span>
            <div class="entry-content">
                <EntryName>{label}</EntryName>
                {modifiers
                    .immune
                    .then(|| {
                        view! {
                            <span class="damage-tag" title=move || i18n.tr("damage-immunity")>
                                <Icon name="shield-check" />
                            </span>
                        }
                    })}
                {modifiers
                    .resistant
                    .then(|| {
                        view! {
                            <span class="damage-tag" title=move || i18n.tr("damage-resistance")>
                                <Icon name="shield-half" />
                            </span>
                        }
                    })}
                {modifiers
                    .vulnerable
                    .then(|| {
                        view! {
                            <span class="damage-tag" title=move || i18n.tr("damage-vulnerability")>
                                <Icon name="shield-off" />
                            </span>
                        }
                    })}
                {(modifiers.reduction > 0)
                    .then(|| {
                        view! {
                            <span class="damage-tag" title=move || i18n.tr("damage-reduction")>
                                <Icon name="shield-minus" />
                                {modifiers.reduction}
                            </span>
                        }
                    })}
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
                <h4 class="session-subsection-title">{move_tr!("session-damage-modifiers")}</h4>
                <div class="entry-list">{entries}</div>
            })
        }
    }
}
