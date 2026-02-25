use leptos::prelude::*;
use strum::IntoEnumIterator;

use crate::{
    components::panel::Panel,
    model::{Character, Proficiency},
};

#[component]
pub fn ProficienciesPanel() -> impl IntoView {
    let char_signal = expect_context::<RwSignal<Character>>();

    let languages = Memo::new(move |_| char_signal.get().languages.clone());
    let racial_traits = Memo::new(move |_| char_signal.get().racial_traits.clone());

    view! {
        <Panel title="Proficiencies & Languages" class="proficiencies-panel">

            // --- Proficiency toggles ---
            <h4>"Proficiencies"</h4>
            <div class="proficiencies-grid">
                {Proficiency::iter()
                    .map(|prof| {
                        let active = Memo::new(move |_| {
                            char_signal.get().proficiencies.get(&prof).copied().unwrap_or(false)
                        });

                        view! {
                            <div class="prof-row">
                                <button
                                    class="prof-toggle"
                                    on:click=move |_| {
                                        char_signal.update(|c| {
                                            let entry = c.proficiencies.entry(prof).or_insert(false);
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
                        .get()
                        .into_iter()
                        .enumerate()
                        .map(|(i, lang)| {
                            view! {
                                <div class="string-list-entry">
                                    <input
                                        type="text"
                                        placeholder="Language"
                                        prop:value=lang
                                        on:input=move |e| {
                                            char_signal.update(|c| {
                                                if let Some(l) = c.languages.get_mut(i) {
                                                    *l = event_target_value(&e);
                                                }
                                            });
                                        }
                                    />
                                    <button
                                        class="btn-remove"
                                        on:click=move |_| {
                                            char_signal.update(|c| {
                                                if i < c.languages.len() {
                                                    c.languages.remove(i);
                                                }
                                            });
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
                    char_signal.update(|c| c.languages.push(String::new()));
                }
            >
                "+ Add Language"
            </button>

            // --- Racial Traits ---
            <h4>"Racial Traits"</h4>
            <div class="string-list">
                {move || {
                    racial_traits
                        .get()
                        .into_iter()
                        .enumerate()
                        .map(|(i, trait_name)| {
                            view! {
                                <div class="string-list-entry">
                                    <input
                                        type="text"
                                        placeholder="Racial trait"
                                        prop:value=trait_name
                                        on:input=move |e| {
                                            char_signal.update(|c| {
                                                if let Some(t) = c.racial_traits.get_mut(i) {
                                                    *t = event_target_value(&e);
                                                }
                                            });
                                        }
                                    />
                                    <button
                                        class="btn-remove"
                                        on:click=move |_| {
                                            char_signal.update(|c| {
                                                if i < c.racial_traits.len() {
                                                    c.racial_traits.remove(i);
                                                }
                                            });
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
                    char_signal.update(|c| c.racial_traits.push(String::new()));
                }
            >
                "+ Add Racial Trait"
            </button>
        </Panel>
    }
}
