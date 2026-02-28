use leptos::prelude::*;
use leptos_fluent::{move_tr, tr};
use leptos_router::components::A;
use reactive_stores::Store;
use strum::IntoEnumIterator;
use wasm_bindgen::prelude::*;

use crate::{
    BASE_URL,
    components::datalist_input::DatalistInput,
    model::{
        Alignment, Character, CharacterIdentityStoreFields, CharacterStoreFields, ClassLevel,
        Translatable,
    },
    rules::RulesRegistry,
    share,
};

fn export_character(character: &Character) {
    let json = match serde_json::to_string_pretty(character) {
        Ok(j) => j,
        Err(error) => {
            log::error!("Failed to serialize character: {error}");
            return;
        }
    };

    let array = js_sys::Array::new();
    array.push(&JsValue::from_str(&json));

    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type("application/json");

    let blob = match web_sys::Blob::new_with_str_sequence_and_options(&array, &opts) {
        Ok(b) => b,
        Err(error) => {
            log::error!("Failed to create blob: {error:?}");
            return;
        }
    };

    let url = match web_sys::Url::create_object_url_with_blob(&blob) {
        Ok(u) => u,
        Err(error) => {
            log::error!("Failed to create object URL: {error:?}");
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
        let Some(files) = input_clone.files() else {
            return;
        };
        let Some(file) = files.get(0) else {
            return;
        };

        let reader = match web_sys::FileReader::new() {
            Ok(r) => r,
            Err(error) => {
                log::error!("Failed to create FileReader: {error:?}");
                return;
            }
        };

        let reader_clone = reader.clone();
        let onload = Closure::<dyn Fn()>::new(move || {
            let result = match reader_clone.result() {
                Ok(r) => r,
                Err(error) => {
                    log::error!("Failed to read file: {error:?}");
                    return;
                }
            };
            let Some(text) = result.as_string() else {
                log::error!("File result is not a string");
                return;
            };
            match serde_json::from_str::<Character>(&text) {
                Ok(mut imported) => {
                    let current_id = store.get_untracked().id;
                    imported.id = current_id;
                    store.set(imported);
                }
                Err(error) => {
                    log::error!("Failed to parse character JSON: {error}");
                    leptos::prelude::window()
                        .alert_with_message(&format!("Invalid character file: {error}"))
                        .ok();
                }
            }
        });

        reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget();

        if let Err(error) = reader.read_as_text(&file) {
            log::error!("Failed to start reading file: {error:?}");
        }
    });

    input.set_onchange(Some(closure.as_ref().unchecked_ref()));
    closure.forget();

    input.click();
}

fn apply_level(store: Store<Character>, registry: RulesRegistry, class_index: usize, level: u32) {
    let class_name = {
        let classes = store.identity().classes().read();
        let Some(cl) = classes.get(class_index) else {
            return;
        };
        cl.class.clone()
    };

    let Some(def) = registry.get_class(&class_name) else {
        return;
    };

    let subclass = {
        let classes = store.identity().classes().read();
        classes.get(class_index).and_then(|c| c.subclass.clone())
    };

    let slots: Option<Vec<u32>> = def
        .features(subclass.as_deref())
        .filter_map(|f| f.spells.as_ref())
        .find_map(|s| s.levels.get(level as usize - 1))
        .and_then(|l| l.slots.clone());

    store.update(|c| {
        def.apply_level(level, c);

        // Re-apply race features at new total level (unlocks level-gated spells)
        if c.identity.race_applied
            && let Some(race_def) = registry.get_race(&c.identity.race)
        {
            let total_level = c.level();
            for feat in race_def.features.values() {
                feat.apply(total_level, c);
            }
        }

        c.update_spell_slots(slots.as_deref());
    });
}

#[component]
pub fn CharacterHeader() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

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

    let share_copied = RwSignal::new(false);

    let on_share = move |_| {
        let encoded = share::encode_character(&store.get());
        let origin = leptos::prelude::window()
            .location()
            .origin()
            .unwrap_or_default();
        let url = format!("{origin}{BASE_URL}/s/{encoded}");

        let clipboard = leptos::prelude::window().navigator().clipboard();
        let promise = clipboard.write_text(&url);
        wasm_bindgen_futures::spawn_local(async move {
            let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
            share_copied.set(true);
        });
        let cb = Closure::once_into_js(move || share_copied.set(false));
        let _ = web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                cb.as_ref().unchecked_ref(),
                2_000,
            );
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
                <div class="header-field race-field">
                    <label>{move_tr!("race")}</label>
                    {move || {
                        let race_name = store.identity().race().get();
                        let race_applied = store.identity().race_applied().get();

                        let race_options: Vec<(String, String)> = registry.with_race_entries(|entries| {
                            entries.iter().map(|entry| {
                                (entry.name.clone(), entry.description.clone())
                            }).collect()
                        });

                        if !race_name.is_empty() {
                            registry.fetch_race(&race_name);
                        }

                        let race_def = registry.get_race(&race_name);
                        let show_apply = race_def.is_some() && !race_applied;

                        view! {
                            <div class="race-input-row">
                                <DatalistInput
                                    value=race_name
                                    placeholder=tr!("race")
                                    options=race_options
                                    on_input=move |name: String| {
                                        let old = store.identity().race().get_untracked();
                                        store.identity().race().set(name.clone());
                                        if name != old {
                                            store.identity().race_applied().set(false);
                                        }
                                        if registry.with_race_entries(|entries| entries.iter().any(|e| e.name == name)) {
                                            registry.fetch_race(&name);
                                        }
                                    }
                                />
                                {if show_apply {
                                    let title = tr!("btn-apply-race");
                                    Some(view! {
                                        <button
                                            class="btn-apply-level"
                                            title=title
                                            on:click=move |_| {
                                                let race_name = store.identity().race().get_untracked();
                                                if let Some(def) = registry.get_race(&race_name) {
                                                    store.update(|c| def.apply(c));
                                                }
                                            }
                                        >
                                            "⬆"
                                        </button>
                                    })
                                } else {
                                    None
                                }}
                            </div>
                        }
                    }}
                </div>
                <div class="header-field background-field">
                    <label>{move_tr!("background")}</label>
                    {move || {
                        let bg_name = store.identity().background().get();
                        let bg_applied = store.identity().background_applied().get();

                        let bg_options: Vec<(String, String)> = registry.with_background_entries(|entries| {
                            entries.iter().map(|entry| {
                                (entry.name.clone(), entry.description.clone())
                            }).collect()
                        });

                        if !bg_name.is_empty() {
                            registry.fetch_background(&bg_name);
                        }

                        let bg_def = registry.get_background(&bg_name);
                        let show_apply = bg_def.is_some() && !bg_applied;

                        view! {
                            <div class="race-input-row">
                                <DatalistInput
                                    value=bg_name
                                    placeholder=tr!("background")
                                    options=bg_options
                                    on_input=move |name: String| {
                                        let old = store.identity().background().get_untracked();
                                        store.identity().background().set(name.clone());
                                        if name != old {
                                            store.identity().background_applied().set(false);
                                        }
                                        if registry.with_background_entries(|entries| entries.iter().any(|e| e.name == name)) {
                                            registry.fetch_background(&name);
                                        }
                                    }
                                />
                                {if show_apply {
                                    let title = tr!("btn-apply-background");
                                    Some(view! {
                                        <button
                                            class="btn-apply-level"
                                            title=title
                                            on:click=move |_| {
                                                let bg_name = store.identity().background().get_untracked();
                                                if let Some(def) = registry.get_background(&bg_name) {
                                                    store.update(|c| def.apply(c));
                                                }
                                            }
                                        >
                                            "⬆"
                                        </button>
                                    })
                                } else {
                                    None
                                }}
                            </div>
                        }
                    }}
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
                                    <option value=val selected=selected>
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
                        let abilities = store.abilities().get();
                        let class_options: Vec<(String, String)> = registry.with_class_entries(|entries| {
                            entries.iter().filter(|entry| {
                                entry.prerequisites.iter().all(|&ability| abilities.get(ability) >= 13)
                            }).map(|entry| {
                                (entry.name.clone(), entry.description.clone())
                            }).collect()
                        });

                        classes
                            .read()
                            .iter()
                            .enumerate()
                            .map(|(i, cl)| {
                                let class_name = cl.class.clone();
                                let subclass_name = cl.subclass.clone().unwrap_or_default();
                                let level_val = cl.level.to_string();
                                let hit_die_val = cl.hit_die_sides.to_string();
                                let current_level = cl.level;
                                let applied = cl.applied_levels.clone();
                                let class_options = class_options.clone();

                                // Trigger lazy fetch if definition not yet loaded
                                if !class_name.is_empty() {
                                    registry.fetch_class(&class_name);
                                }

                                let class_def = registry.get_class(&class_name);

                                let next_unapplied: Option<u32> = class_def.as_ref()
                                    .and_then(|_| {
                                        (1..=current_level)
                                            .find(|lvl| !applied.contains(lvl))
                                    });

                                let subclass_options: Vec<(String, String)> = class_def
                                    .as_ref()
                                    .map(|def| {
                                        def.subclasses
                                            .values()
                                            .filter(|sc| sc.min_level() <= current_level)
                                            .map(|sc| (sc.name.clone(), sc.description.clone()))
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                let has_subclasses = !subclass_options.is_empty();

                                view! {
                                    <div class="class-entry">
                                        <DatalistInput
                                            value=class_name
                                            placeholder=tr!("class")
                                            class="class-name"
                                            options=class_options
                                            on_input=move |name: String| {
                                                classes.write()[i].class.clone_from(&name);
                                                if registry.with_class_entries(|entries| entries.iter().any(|e| e.name == name)) {
                                                    registry.fetch_class(&name);
                                                    if let Some(def) = registry.get_class(&name) {
                                                        classes.write()[i].hit_die_sides = def.hit_die;
                                                    }
                                                }
                                            }
                                        />
                                        {if has_subclasses {
                                            Some(view! {
                                                <DatalistInput
                                                    value=subclass_name
                                                    placeholder=tr!("subclass")
                                                    class="class-subclass"
                                                    options=subclass_options
                                                    on_input=move |val: String| {
                                                        classes.write()[i].subclass = if val.is_empty() { None } else { Some(val) };
                                                    }
                                                />
                                            })
                                        } else {
                                            None
                                        }}
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
                                        {if let Some(lvl) = next_unapplied {
                                            let title = tr!("btn-apply-level", {"level" => lvl.to_string()});
                                            Some(view! {
                                                <div class="apply-levels">
                                                    <button
                                                        class="btn-apply-level"
                                                        title=title
                                                        on:click=move |_| {
                                                            apply_level(store, registry, i, lvl);
                                                        }
                                                    >
                                                        {format!("⬆{lvl}")}
                                                    </button>
                                                </div>
                                            })
                                        } else {
                                            None
                                        }}
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
                <button class="btn-add" on:click=on_share>
                    {move || if share_copied.get() { tr!("share-copied") } else { tr!("share-link") }}
                </button>
                <button class="btn-add" on:click=on_export>{move_tr!("export-json")}</button>
                <button class="btn-add" on:click=on_import>{move_tr!("import-json")}</button>
                <button
                    class="btn-add btn-danger"
                    on:click=move |_| {
                        let window = web_sys::window().unwrap();
                        if window.confirm_with_message("Reset character to blank?").unwrap_or(false) {
                            store.set(Character { id: store.get().id, ..Default::default() });
                        }
                    }
                >
                    {move_tr!("reset-character")}
                </button>
            </div>
            <A href=format!("{BASE_URL}/") attr:class="back-link">{move_tr!("back-to-characters")}</A>
        </div>
    }
}
