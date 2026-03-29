use leptos::prelude::*;
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;

use crate::{
    components::icon::Icon,
    model::{Character, CharacterStoreFields, Translatable},
};

#[component]
pub fn ResistancesBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let i18n = expect_context::<I18n>();
    let resistances = store.resistances();

    move || {
        let entries: Vec<_> = resistances
            .read()
            .iter()
            .filter(|(_, mods)| mods.is_active())
            .map(|(dt, mods)| {
                let icon = dt.icon_name();
                let label = i18n.tr(dt.tr_key());
                let resistant = mods.resistant;
                let vulnerable = mods.vulnerable;
                let immune = mods.immune;
                let reduction = mods.reduction;
                view! {
                    <span class="damage-entry">
                        <Icon name=icon size=14 />
                        {label}
                        {immune.then(|| view! {
                            <span class="damage-tag damage-immunity" title=move || i18n.tr("damage-immunity")>
                                <Icon name="shield-check" size=12 />
                            </span>
                        })}
                        {resistant.then(|| view! {
                            <span class="damage-tag damage-resistance" title=move || i18n.tr("damage-resistance")>
                                <Icon name="shield-half" size=12 />
                            </span>
                        })}
                        {vulnerable.then(|| view! {
                            <span class="damage-tag damage-vulnerability" title=move || i18n.tr("damage-vulnerability")>
                                <Icon name="shield-off" size=12 />
                            </span>
                        })}
                        {(reduction > 0).then(|| view! {
                            <span class="damage-tag damage-reduction" title=move || i18n.tr("damage-reduction")>
                                <Icon name="shield-minus" size=12 />
                                {reduction}
                            </span>
                        })}
                    </span>
                }
            })
            .collect();

        if entries.is_empty() {
            None
        } else {
            Some(view! {
                <h4 class="summary-subsection-title">{move_tr!("summary-resistances")}</h4>
                <div class="summary-resistances">{entries.collect_view()}</div>
            })
        }
    }
}
