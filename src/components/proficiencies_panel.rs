use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::panel::Panel,
    model::{Character, CharacterStoreFields, Proficiency, Translatable},
};

#[component]
pub fn ProficienciesPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let languages = store.languages();
    let i18n = expect_context::<leptos_fluent::I18n>();

    view! {
        <Panel title=move_tr!("panel-proficiencies") class="proficiencies-panel">

            // --- Proficiency toggles ---
            <h4>{move_tr!("proficiencies")}</h4>
            <div class="proficiencies-grid">
                {Proficiency::iter()
                    .map(|prof| {
                        let active = Memo::new(move |_| {
                            store.proficiencies().read().contains(&prof)
                        });
                        let tr_key = prof.tr_key();
                        let label = Signal::derive(move || i18n.tr(tr_key));

                        view! {
                            <div class="prof-row">
                                <button
                                    class="prof-toggle"
                                    on:click=move |_| {
                                        store.proficiencies().update(|profs| {
                                            if !profs.remove(&prof) {
                                                profs.insert(prof);
                                            }
                                        });
                                    }
                                >
                                    {move || if active.get() { "\u{25CF}" } else { "\u{25CB}" }}
                                </button>
                                <span class="prof-label">{label}</span>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>

            // --- Languages ---
            <h4>{move_tr!("languages")}</h4>
            <div class="string-list">
                {move || {
                    languages
                        .read()
                        .iter()
                        .enumerate()
                        .map(|(i, lang)| {
                            let val = lang.clone();
                            view! {
                                <div class="string-list-entry">
                                    <input
                                        type="text"
                                        placeholder=move_tr!("language")
                                        prop:value=val
                                        on:input=move |e| {
                                            languages.write().set(i, event_target_value(&e));
                                        }
                                    />
                                    <button
                                        class="btn-remove"
                                        on:click=move |_| {
                                            if i < languages.read().len() {
                                                languages.write().remove_at(i);
                                            }
                                        }
                                    >
                                        "X"
                                    </button>
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
            <button
                class="btn-add"
                on:click=move |_| {
                    languages.write().push(String::new());
                }
            >
                {move_tr!("btn-add-language")}
            </button>
        </Panel>
    }
}
