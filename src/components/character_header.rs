use leptos::prelude::*;
use leptos_router::components::A;
use strum::IntoEnumIterator;
use wasm_bindgen::prelude::*;

use crate::model::{Alignment, Character, ClassLevel};

fn export_character(character: &Character) {
    let json = match serde_json::to_string_pretty(character) {
        Ok(j) => j,
        Err(e) => {
            log::error!("Failed to serialize character: {e}");
            return;
        }
    };

    let array = js_sys::Array::new();
    array.push(&JsValue::from_str(&json));

    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type("application/json");

    let blob = match web_sys::Blob::new_with_str_sequence_and_options(&array, &opts) {
        Ok(b) => b,
        Err(e) => {
            log::error!("Failed to create blob: {e:?}");
            return;
        }
    };

    let url = match web_sys::Url::create_object_url_with_blob(&blob) {
        Ok(u) => u,
        Err(e) => {
            log::error!("Failed to create object URL: {e:?}");
            return;
        }
    };

    let document = leptos::prelude::document();
    let anchor: web_sys::HtmlAnchorElement = document.create_element("a").unwrap().unchecked_into();

    let filename = if character.identity.name.is_empty() {
        "character.json".to_string()
    } else {
        format!("{}.json", character.identity.name)
    };

    anchor.set_href(&url);
    anchor.set_download(&filename);
    anchor.click();

    let _ = web_sys::Url::revoke_object_url(&url);
}

fn import_character(char_signal: RwSignal<Character>) {
    let document = leptos::prelude::document();
    let input: web_sys::HtmlInputElement =
        document.create_element("input").unwrap().unchecked_into();

    input.set_type("file");
    input.set_accept(".json");

    let input_clone = input.clone();
    let closure = Closure::<dyn Fn()>::new(move || {
        let files = match input_clone.files() {
            Some(f) => f,
            None => return,
        };
        let file = match files.get(0) {
            Some(f) => f,
            None => return,
        };

        let reader = match web_sys::FileReader::new() {
            Ok(r) => r,
            Err(e) => {
                log::error!("Failed to create FileReader: {e:?}");
                return;
            }
        };

        let reader_clone = reader.clone();
        let onload = Closure::<dyn Fn()>::new(move || {
            let result = match reader_clone.result() {
                Ok(r) => r,
                Err(e) => {
                    log::error!("Failed to read file: {e:?}");
                    return;
                }
            };
            let text = match result.as_string() {
                Some(t) => t,
                None => {
                    log::error!("File result is not a string");
                    return;
                }
            };
            match serde_json::from_str::<Character>(&text) {
                Ok(mut imported) => {
                    let current_id = char_signal.get_untracked().id;
                    imported.id = current_id;
                    char_signal.set(imported);
                }
                Err(e) => {
                    log::error!("Failed to parse character JSON: {e}");
                    leptos::prelude::window()
                        .alert_with_message(&format!("Invalid character file: {e}"))
                        .ok();
                }
            }
        });

        reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget();

        if let Err(e) = reader.read_as_text(&file) {
            log::error!("Failed to start reading file: {e:?}");
        }
    });

    input.set_onchange(Some(closure.as_ref().unchecked_ref()));
    closure.forget();

    input.click();
}

#[component]
pub fn CharacterHeader() -> impl IntoView {
    let char_signal = expect_context::<RwSignal<Character>>();

    let name = Memo::new(move |_| char_signal.get().identity.name.clone());
    let classes = Memo::new(move |_| char_signal.get().identity.classes.clone());
    let total_level = Memo::new(move |_| char_signal.get().level());
    let race = Memo::new(move |_| char_signal.get().identity.race.clone());
    let background = Memo::new(move |_| char_signal.get().identity.background.clone());
    let alignment = Memo::new(move |_| char_signal.get().identity.alignment);
    let xp = Memo::new(move |_| char_signal.get().identity.experience_points);
    let prof_bonus = Memo::new(move |_| char_signal.get().proficiency_bonus());

    let add_class = move |_| {
        char_signal.update(|c| c.identity.classes.push(ClassLevel::default()));
    };

    let on_export = move |_| {
        export_character(&char_signal.get());
    };

    let on_import = move |_| {
        import_character(char_signal);
    };

    view! {
        <div class="panel character-header">
            <div class="header-row">
                <div class="header-field name-field">
                    <label>"Character Name"</label>
                    <input
                        type="text"
                        prop:value=name
                        on:input=move |e| {
                            char_signal.update(|c| c.identity.name = event_target_value(&e));
                        }
                    />
                </div>
                <div class="header-field">
                    <label>"Race"</label>
                    <input
                        type="text"
                        prop:value=race
                        on:input=move |e| {
                            char_signal.update(|c| c.identity.race = event_target_value(&e));
                        }
                    />
                </div>
                <div class="header-field">
                    <label>"Background"</label>
                    <input
                        type="text"
                        prop:value=background
                        on:input=move |e| {
                            char_signal.update(|c| c.identity.background = event_target_value(&e));
                        }
                    />
                </div>
                <div class="header-field">
                    <label>"Alignment"</label>
                    <select
                        on:change=move |e| {
                            let val = event_target_value(&e);
                            if let Ok(a) = serde_json::from_str::<Alignment>(&format!("\"{val}\"")) {
                                char_signal.update(|c| c.identity.alignment = a);
                            }
                        }
                    >
                        {Alignment::iter()
                            .map(|a| {
                                let label = a.to_string();
                                let val = format!("{a:?}");
                                let selected = move || alignment.get() == a;
                                view! {
                                    <option value=val.clone() selected=selected>
                                        {label}
                                    </option>
                                }
                            })
                            .collect_view()}
                    </select>
                </div>
                <div class="header-field level-field">
                    <label>"XP"</label>
                    <input
                        type="number"
                        min="0"
                        prop:value=move || xp.get().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                char_signal.update(|c| c.identity.experience_points = v);
                            }
                        }
                    />
                </div>
                <div class="header-field level-field">
                    <label>"Total Level"</label>
                    <span class="computed-value">{total_level}</span>
                </div>
                <div class="header-field level-field">
                    <label>"Prof. Bonus"</label>
                    <span class="computed-value">"+" {prof_bonus}</span>
                </div>
            </div>

            <div class="classes-section">
                <label>"Classes"</label>
                <div class="classes-list">
                    {move || {
                        classes
                            .get()
                            .into_iter()
                            .enumerate()
                            .map(|(i, cl)| {
                                view! {
                                    <div class="class-entry">
                                        <input
                                            type="text"
                                            class="class-name"
                                            placeholder="Class"
                                            prop:value=cl.class.clone()
                                            on:input=move |e| {
                                                char_signal.update(|c| {
                                                    if let Some(entry) = c.identity.classes.get_mut(i) {
                                                        entry.class = event_target_value(&e);
                                                    }
                                                });
                                            }
                                        />
                                        <input
                                            type="number"
                                            class="class-level"
                                            min="1"
                                            max="20"
                                            prop:value=cl.level.to_string()
                                            on:input=move |e| {
                                                if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                                    char_signal.update(|c| {
                                                        if let Some(entry) = c.identity.classes.get_mut(i) {
                                                            entry.level = v.clamp(1, 20);
                                                        }
                                                    });
                                                }
                                            }
                                        />
                                        <Show when={move || classes.get().len() > 1}>
                                            <button
                                                class="btn-remove"
                                                on:click=move |_| {
                                                    char_signal.update(|c| {
                                                        if c.identity.classes.len() > 1 {
                                                            c.identity.classes.remove(i);
                                                        }
                                                    });
                                                }
                                            >
                                                "X"
                                            </button>
                                        </Show>
                                    </div>
                                }
                            })
                            .collect_view()
                    }}
                </div>
                <button class="btn-add btn-add-class" on:click=add_class>
                    "+ Add Class"
                </button>
            </div>

            <div class="header-actions">
                <A href=format!("{}/", crate::BASE_URL) attr:class="back-link">"< Back to Characters"</A>
                <button class="btn-add" on:click=on_export>"Export JSON"</button>
                <button class="btn-add" on:click=on_import>"Import JSON"</button>
            </div>
        </div>
    }
}
