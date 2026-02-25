use leptos::prelude::*;
use leptos_fluent::{move_tr, tr};
use leptos_router::components::A;
use reactive_stores::Store;
use strum::IntoEnumIterator;
use wasm_bindgen::prelude::*;

use crate::model::{
    Alignment, Character, CharacterIdentityStoreFields, CharacterStoreFields, ClassLevel,
    Translatable,
};

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

fn import_character(store: Store<Character>) {
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
                    let current_id = store.get().id;
                    imported.id = current_id;
                    store.set(imported);
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
    let store = expect_context::<Store<Character>>();

    let total_level = Memo::new(move |_| store.get().level());
    let prof_bonus = Memo::new(move |_| store.get().proficiency_bonus());

    let classes = store.identity().classes();

    let add_class = move |_| {
        classes.write().push(ClassLevel::default());
    };

    let on_export = move |_| {
        export_character(&store.get());
    };

    let on_import = move |_| {
        import_character(store);
    };

    let i18n = expect_context::<leptos_fluent::I18n>();

    view! {
        <div class="panel character-header">
            <div class="header-row">
                <div class="header-field name-field">
                    <label>{move_tr!("character-name")}</label>
                    <input
                        type="text"
                        prop:value=move || store.identity().name().get()
                        on:input=move |e| {
                            store.identity().name().set(event_target_value(&e));
                        }
                    />
                </div>
                <div class="header-field">
                    <label>{move_tr!("race")}</label>
                    <input
                        type="text"
                        prop:value=move || store.identity().race().get()
                        on:input=move |e| {
                            store.identity().race().set(event_target_value(&e));
                        }
                    />
                </div>
                <div class="header-field">
                    <label>{move_tr!("background")}</label>
                    <input
                        type="text"
                        prop:value=move || store.identity().background().get()
                        on:input=move |e| {
                            store.identity().background().set(event_target_value(&e));
                        }
                    />
                </div>
                <div class="header-field">
                    <label>{move_tr!("alignment")}</label>
                    <select
                        on:change=move |e| {
                            let val = event_target_value(&e);
                            if let Ok(a) = serde_json::from_str::<Alignment>(&format!("\"{val}\"")) {
                                store.identity().alignment().set(a);
                            }
                        }
                    >
                        {Alignment::iter()
                            .map(|a| {
                                let tr_key = a.tr_key();
                                let val = format!("{a:?}");
                                let selected = move || store.identity().alignment().get() == a;
                                let label = Signal::derive(move || i18n.tr(tr_key));
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
                    <label>{move_tr!("xp")}</label>
                    <input
                        type="number"
                        min="0"
                        prop:value=move || store.identity().experience_points().get().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                store.identity().experience_points().set(v);
                            }
                        }
                    />
                </div>
                <div class="header-field level-field">
                    <label>{move_tr!("total-level")}</label>
                    <span class="computed-value">{total_level}</span>
                </div>
                <div class="header-field level-field">
                    <label>{move_tr!("prof-bonus")}</label>
                    <span class="computed-value">"+" {prof_bonus}</span>
                </div>
            </div>

            <div class="classes-section">
                <label>{move_tr!("classes")}</label>
                <div class="classes-list">
                    {move || {
                        classes
                            .read()
                            .iter()
                            .enumerate()
                            .map(|(i, cl)| {
                                let class_name = cl.class.clone();
                                let level_val = cl.level.to_string();
                                let hit_die_val = cl.hit_die_sides.to_string();
                                view! {
                                    <div class="class-entry">
                                        <input
                                            type="text"
                                            class="class-name"
                                            placeholder=tr!("class")
                                            prop:value=class_name
                                            on:input=move |e| {
                                                classes.write()[i].class = event_target_value(&e);
                                            }
                                        />
                                        <select
                                            class="class-hit-die"
                                            prop:value=hit_die_val
                                            on:change=move |e| {
                                                if let Ok(v) = event_target_value(&e).parse::<u16>() {
                                                    classes.write()[i].hit_die_sides = v;
                                                }
                                            }
                                        >
                                            <option value="6" selected=move || classes.read()[i].hit_die_sides == 6>"d6"</option>
                                            <option value="8" selected=move || classes.read()[i].hit_die_sides == 8>"d8"</option>
                                            <option value="10" selected=move || classes.read()[i].hit_die_sides == 10>"d10"</option>
                                            <option value="12" selected=move || classes.read()[i].hit_die_sides == 12>"d12"</option>
                                        </select>
                                        <input
                                            type="number"
                                            class="class-level"
                                            min="1"
                                            max="20"
                                            prop:value=level_val
                                            on:input=move |e| {
                                                if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                                    classes.write()[i].level = v.clamp(1, 20);
                                                }
                                            }
                                        />
                                        <Show when={move || classes.read().len() > 1}>
                                            <button
                                                class="btn-remove"
                                                on:click=move |_| {
                                                    if classes.read().len() > 1 {
                                                        classes.write().remove(i);
                                                    }
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
                    {move_tr!("btn-add-class")}
                </button>
            </div>

            <div class="header-actions">
                <A href="/" attr:class="back-link">{move_tr!("back-to-characters")}</A>
                <button class="btn-add" on:click=on_export>{move_tr!("export-json")}</button>
                <button class="btn-add" on:click=on_import>{move_tr!("import-json")}</button>
                <LanguageSwitcher />
            </div>
        </div>
    }
}

#[component]
fn LanguageSwitcher() -> impl IntoView {
    let i18n = expect_context::<leptos_fluent::I18n>();

    view! {
        <div class="lang-switcher">
            {i18n.languages.iter().map(|lang| {
                let lang = *lang;
                let is_active = move || i18n.language.get() == lang;
                view! {
                    <button
                        class="lang-btn"
                        class:active=is_active
                        on:click=move |_| {
                            i18n.language.set(lang);
                        }
                    >
                        {lang.id.to_string().to_uppercase()}
                    </button>
                }
            }).collect_view()}
        </div>
    }
}
