use leptos::prelude::*;
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::slot_box::SlotBox,
    model::{Character, CharacterCoreStoreFields, CharacterStoreFields, Sense, Translatable},
};

#[component]
pub fn SensesPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let i18n = expect_context::<I18n>();
    let expanded = RwSignal::new(false);

    view! {
        <section>
            <div class="section-header">
                <button
                    class="btn-toggle-desc"
                    class:expanded=move || expanded.get()
                    on:click=move |_| expanded.update(|value| *value = !*value)
                />
                <h3 class="clickable" on:click=move |_| expanded.update(|value| *value = !*value)>
                    {move_tr!("senses")}
                </h3>
            </div>
            <div class="slot-box-list">
                {move || {
                    let is_expanded = expanded.get();
                    Sense::iter()
                        .filter(move |sense| {
                            is_expanded || store.core().senses().read().get(*sense) > 0
                        })
                        .map(|sense| {
                            let current = Memo::new(move |_| {
                                store.core().senses().read().get(sense)
                            });
                            let tr_key = sense.tr_key();
                            let label = Signal::derive(move || i18n.tr(tr_key));
                            let icon = sense.icon_name();
                            view! {
                                <SlotBox label=label icon=icon>
                                    <input
                                        type="number"
                                        min="0"
                                        prop:value=move || current.get()
                                        on:input=move |event| {
                                            let feet = event_target_value(&event)
                                                .parse::<u32>()
                                                .unwrap_or(0);
                                            store
                                                .core()
                                                .senses()
                                                .update(|senses| senses.set(sense, feet));
                                        }
                                    />
                                </SlotBox>
                            }
                        })
                        .collect_view()
                }}
            </div>
        </section>
    }
}
