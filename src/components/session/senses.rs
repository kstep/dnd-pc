use leptos::prelude::*;
use leptos_fluent::{I18n, move_tr};
use strum::IntoEnumIterator;

use crate::{
    components::{entry_name::EntryName, icon::Icon},
    effective::EffectiveCharacter,
    model::{Sense, Translatable},
};

#[component]
pub fn SensesBlock() -> impl IntoView {
    let effective = expect_context::<EffectiveCharacter>();
    let i18n = expect_context::<I18n>();

    move || {
        let senses = effective.senses();
        let entries = Sense::iter()
            .filter_map(|sense| {
                let feet = senses.get(sense);
                if feet == 0 {
                    return None;
                }
                let icon = sense.icon_name();
                let tr_key = sense.tr_key();
                let title = untrack(|| i18n.tr(tr_key).into_owned());
                let label = Signal::derive(move || i18n.tr(tr_key).into_owned());
                Some(view! {
                    <div class="entry-item">
                        <span class="damage-dt-icon">
                            <Icon name=icon title=title />
                        </span>
                        <div class="entry-content">
                            <EntryName>{label}</EntryName>
                            <span class="damage-tag">{feet}" ft"</span>
                        </div>
                    </div>
                })
            })
            .collect_view();

        if entries.is_empty() {
            None
        } else {
            Some(view! {
                <h4 class="session-subsection-title">{move_tr!("session-senses")}</h4>
                <div class="entry-list">{entries}</div>
            })
        }
    }
}
