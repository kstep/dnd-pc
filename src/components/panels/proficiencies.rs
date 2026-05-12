use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::{icon::Icon, slot_box::SlotBox},
    model::{
        Ability, Character, CharacterCoreStoreFields, CharacterStoreFields, Proficiency,
        Translatable,
    },
};

#[component]
pub fn ProficienciesPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let languages = store.core().languages();
    let tools = store.core().tools();
    let i18n = expect_context::<leptos_fluent::I18n>();

    view! {
        <section>
            <h3>{move_tr!("proficiencies")}</h3>
            <div class="slot-box-list">
                {Proficiency::iter()
                    .map(|prof| {
                        let active = Memo::new(move |_| {
                            store.core().proficiencies().read().contains(&prof)
                        });
                        let tr_key = prof.tr_key();
                        let label = Signal::derive(move || i18n.tr(tr_key));

                        view! {
                            <SlotBox
                                label=label
                                class:clickable=true
                                class:tag=true
                                class:highlighted=move || active.get()
                                on:click=move |_| {
                                    store.core().proficiencies().update(|profs| profs.toggle(prof));
                                }
                            >
                                <Icon
                                    name=Signal::derive(move || if active.get() { "circle-dot" } else { "circle-dashed" })
                                />
                            </SlotBox>
                        }
                    })
                    .collect_view()}
            </div>
        </section>

        <section>
            <h3>{move_tr!("languages")}</h3>
            <div class="entry-list">
                {move || {
                    languages
                        .read()
                        .iter()
                        .enumerate()
                        .map(|(i, lang)| {
                            let val = lang.clone();
                            view! {
                                <div class="entry-item">
                                    <div class="entry-content">
                                        <input
                                            type="text"
                                            class="entry-name"
                                            placeholder=move_tr!("language")
                                            prop:value=val
                                            on:input=move |e| {
                                                languages.write().set(i, event_target_value(&e));
                                            }
                                        />
                                    </div>
                                    <div class="entry-actions">
                                        <button
                                            class="btn-remove"
                                            on:click=move |_| {
                                                if i < languages.read().len() {
                                                    languages.write().remove_at(i);
                                                }
                                            }
                                        >
                                            <Icon name="x" />
                                        </button>
                                    </div>
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
            <button
                class="btn-primary"
                on:click=move |_| {
                    languages.write().push(String::new());
                }
            >
                {move_tr!("btn-add-language")}
            </button>
        </section>

        <section>
            <h3>{move_tr!("tools")}</h3>
            <div class="entry-list">
                {move || {
                    tools
                        .read()
                        .iter()
                        .enumerate()
                        .map(|(i, entry)| {
                            let current_name = entry.name.clone();
                            let current_prof = entry.prof;
                            let current_ability = entry.ability;
                            view! {
                                <div class="entry-item">
                                    <div class="entry-content">
                                        <button
                                            class="btn-icon tool-tier"
                                            title=current_prof.icon_name()
                                            on:click=move |_| {
                                                if let Some(entry) = tools.write().get_mut(i) {
                                                    entry.prof = entry.prof.next();
                                                }
                                            }
                                        >
                                            <Icon name=current_prof.icon_name() />
                                        </button>
                                        <input
                                            type="text"
                                            class="entry-name"
                                            placeholder=move_tr!("tool")
                                            prop:value=current_name
                                            on:input=move |e| {
                                                if let Some(entry) = tools.write().get_mut(i) {
                                                    entry.name = event_target_value(&e);
                                                }
                                            }
                                        />
                                        <select
                                            class="tool-ability"
                                            on:change=move |e| {
                                                let Some(ability) = Ability::from_u8_str(&event_target_value(&e)) else { return };
                                                if let Some(entry) = tools.write().get_mut(i) {
                                                    entry.ability = ability;
                                                }
                                            }
                                        >
                                            {Ability::iter()
                                                .map(|ability| {
                                                    let option_value = (ability as u8).to_string();
                                                    let selected = ability == current_ability;
                                                    let tr_key = ability.tr_key();
                                                    let label = Signal::derive(move || i18n.tr(tr_key));
                                                    view! {
                                                        <option value=option_value selected=selected>
                                                            {label}
                                                        </option>
                                                    }
                                                })
                                                .collect_view()}
                                        </select>
                                    </div>
                                    <div class="entry-actions">
                                        <button
                                            class="btn-remove"
                                            on:click=move |_| {
                                                tools.write().remove(i);
                                            }
                                        >
                                            <Icon name="x" />
                                        </button>
                                    </div>
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
            <button
                class="btn-primary"
                on:click=move |_| {
                    tools.write().push_empty();
                }
            >
                {move_tr!("btn-add-tool")}
            </button>
        </section>
    }
}
