use std::collections::HashSet;

use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::panel::Panel,
    model::{Character, CharacterStoreFields, Proficiency, RacialTrait, Translatable},
};

#[component]
pub fn ProficienciesPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let languages = store.languages();
    let racial_traits = store.racial_traits();
    let i18n = expect_context::<leptos_fluent::I18n>();

    view! {
        <Panel title=move_tr!("panel-proficiencies") class="proficiencies-panel">

            // --- Proficiency toggles ---
            <h4>{move_tr!("proficiencies")}</h4>
            <div class="proficiencies-grid">
                {Proficiency::iter()
                    .map(|prof| {
                        let active = Memo::new(move |_| {
                            store.proficiencies().read().get(&prof).copied().unwrap_or(false)
                        });
                        let tr_key = prof.tr_key();
                        let label = Signal::derive(move || i18n.tr(tr_key));

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
                {move_tr!("btn-add-language")}
            </button>

            // --- Racial Traits ---
            <h4>{move_tr!("racial-traits")}</h4>
            {
                let rt_expanded = RwSignal::new(HashSet::<usize>::new());
                view! {
                    <div class="string-list">
                        {move || {
                            racial_traits
                                .read()
                                .iter()
                                .enumerate()
                                .map(|(i, rt)| {
                                    let name = rt.name.clone();
                                    let desc = rt.description.clone();
                                    let is_open = move || rt_expanded.get().contains(&i);
                                    let toggle = move |_| {
                                        rt_expanded.update(|set| {
                                            if !set.remove(&i) {
                                                set.insert(i);
                                            }
                                        });
                                    };
                                    view! {
                                        <div class="feature-entry">
                                            <input
                                                type="text"
                                                class="feature-name"
                                                placeholder=move_tr!("trait-name")
                                                prop:value=name
                                                on:input=move |e| {
                                                    racial_traits.write()[i].name = event_target_value(&e);
                                                }
                                            />
                                            <button
                                                class="btn-toggle-desc"
                                                on:click=toggle
                                            >
                                                {move || if is_open() { "\u{2212}" } else { "+" }}
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
                                            <Show when=is_open>
                                                <textarea
                                                    class="feature-desc"
                                                    placeholder=move_tr!("description")
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
                }
            }
            <button
                class="btn-add"
                on:click=move |_| {
                    racial_traits.write().push(RacialTrait::default());
                }
            >
                {move_tr!("btn-add-racial-trait")}
            </button>
        </Panel>
    }
}
