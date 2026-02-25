use leptos::prelude::*;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::panel::Panel,
    model::{Character, CharacterStoreFields, Proficiency, RacialTrait},
};

#[component]
pub fn ProficienciesPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let languages = store.languages();
    let racial_traits = store.racial_traits();

    view! {
        <Panel title="Proficiencies & Languages" class="proficiencies-panel">

            // --- Proficiency toggles ---
            <h4>"Proficiencies"</h4>
            <div class="proficiencies-grid">
                {Proficiency::iter()
                    .map(|prof| {
                        let active = Memo::new(move |_| {
                            store.proficiencies().read().get(&prof).copied().unwrap_or(false)
                        });

                        view! {
                            <div class="prof-row">
                                <button
                                    class="prof-toggle"
                                    on:click=move |_| {
                                        store.proficiencies().update(|profs| {
                                            let entry = profs.entry(prof).or_insert(false);
                                            *entry = !*entry;
                                        });
                                    }
                                >
                                    {move || if active.get() { "\u{25CF}" } else { "\u{25CB}" }}
                                </button>
                                <span class="prof-label">{prof.to_string()}</span>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>

            // --- Languages ---
            <h4>"Languages"</h4>
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
                                        placeholder="Language"
                                        prop:value=val
                                        on:input=move |e| {
                                            languages.write()[i] = event_target_value(&e);
                                        }
                                    />
                                    <button
                                        class="btn-remove"
                                        on:click=move |_| {
                                            if i < languages.read().len() {
                                                languages.write().remove(i);
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
                "+ Add Language"
            </button>

            // --- Racial Traits ---
            <h4>"Racial Traits"</h4>
            <div class="string-list">
                {move || {
                    racial_traits
                        .read()
                        .iter()
                        .enumerate()
                        .map(|(i, rt)| {
                            let name = rt.name.clone();
                            let desc = rt.description.clone();
                            let show_desc = RwSignal::new(false);
                            view! {
                                <div class="feature-entry">
                                    <input
                                        type="text"
                                        class="feature-name"
                                        placeholder="Trait name"
                                        prop:value=name
                                        on:input=move |e| {
                                            racial_traits.write()[i].name = event_target_value(&e);
                                        }
                                    />
                                    <button
                                        class="btn-toggle-desc"
                                        on:click=move |_| show_desc.update(|v| *v = !*v)
                                    >
                                        {move || if show_desc.get() { "\u{2212}" } else { "+" }}
                                    </button>
                                    <button
                                        class="btn-remove"
                                        on:click=move |_| {
                                            if i < racial_traits.read().len() {
                                                racial_traits.write().remove(i);
                                            }
                                        }
                                    >
                                        "X"
                                    </button>
                                    <Show when=move || show_desc.get()>
                                        <textarea
                                            class="feature-desc"
                                            placeholder="Description"
                                            prop:value=desc.clone()
                                            on:input=move |e| {
                                                racial_traits.write()[i].description = event_target_value(&e);
                                            }
                                        />
                                    </Show>
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
            <button
                class="btn-add"
                on:click=move |_| {
                    racial_traits.write().push(RacialTrait::default());
                }
            >
                "+ Add Racial Trait"
            </button>
        </Panel>
    }
}
