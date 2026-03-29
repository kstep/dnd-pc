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
        let entries = damage_modifiers
            .read()
            .iter()
            .filter(|(_, modifiers)| modifiers.is_active())
            .map(|(damage_type, modifiers)| {
                let icon = damage_type.icon_name();
                let label = i18n.tr(damage_type.tr_key());
                let immune = modifiers.immune;
                let resistant = modifiers.resistant;
                let vulnerable = modifiers.vulnerable;
                let reduction = modifiers.reduction;

                view! {
                    <div class="entry-item">
                        <span class="damage-dt-icon"><Icon name=icon size=14 title=label.clone() /></span>
                        <div class="entry-content">
                            <span class="entry-name">{label}</span>
                            <span class="entry-badge damage-badge">
                                {immune.then(|| view! {
                                    <span class="damage-tag" title=move || i18n.tr("damage-immunity")>
                                        <Icon name="shield-check" size=14 />
                                    </span>
                                })}
                                {resistant.then(|| view! {
                                    <span class="damage-tag" title=move || i18n.tr("damage-resistance")>
                                        <Icon name="shield-half" size=14 />
                                    </span>
                                })}
                                {vulnerable.then(|| view! {
                                    <span class="damage-tag" title=move || i18n.tr("damage-vulnerability")>
                                        <Icon name="shield-off" size=14 />
                                    </span>
                                })}
                                {(reduction > 0).then(|| view! {
                                    <span class="damage-tag" title=move || i18n.tr("damage-reduction")>
                                        <Icon name="shield-minus" size=14 />
                                        {reduction}
                                    </span>
                                })}
                            </span>
                        </div>
                    </div>
                }
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
