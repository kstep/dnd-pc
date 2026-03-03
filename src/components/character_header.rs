use leptos::prelude::*;
use leptos_fluent::{move_tr, tr};
use leptos_router::{components::A, hooks::use_navigate};
use reactive_stores::Store;
use strum::IntoEnumIterator;
use uuid::Uuid;
use wasm_bindgen::prelude::*;

use crate::{
    BASE_URL,
    components::datalist_input::DatalistInput,
    model::{
        Alignment, Character, CharacterIdentityStoreFields, CharacterStoreFields, ClassLevel,
        Translatable,
    },
    rules::RulesRegistry,
    share, storage,
};

fn export_character(character: &Character) {
    let json = match serde_json::to_string_pretty(character) {
        Ok(json) => json,
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
        Ok(blob) => blob,
        Err(error) => {
            log::error!("Failed to create blob: {error:?}");
            return;
        }
    };

    let url = match web_sys::Url::create_object_url_with_blob(&blob) {
        Ok(url) => url,
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
    storage::pick_character_from_file(move |mut imported| {
        let current_id = store.get_untracked().id;
        imported.id = current_id;
        store.set(imported);
    });
}

fn apply_level(store: Store<Character>, registry: RulesRegistry, class_index: usize, level: u32) {
    let (class_name, subclass) = {
        let classes = store.identity().classes().read();
        let Some(class) = classes.get(class_index) else {
            return;
        };
        (class.class.clone(), class.subclass.clone())
    };

    registry.with_class(&class_name, |def| {
        let slots: Option<Vec<u32>> = def
            .features(subclass.as_deref())
            .filter_map(|f| f.spells.as_ref())
            .find_map(|s| s.levels.get(level as usize - 1))
            .and_then(|l| l.slots.clone());

        store.update(|c| {
            def.apply_level(level, c);

            // Re-apply race features at new total level (unlocks level-gated spells)
            if c.identity.race_applied {
                let race_name = c.identity.race.clone();
                registry.with_race(&race_name, |race_def| {
                    let total_level = c.level();
                    for feat in race_def.features.values() {
                        feat.apply(total_level, c);
                    }
                });
            }

            c.update_spell_slots(slots.as_deref());
        });
    });
}

#[component]
pub fn CharacterHeader() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let total_level = Memo::new(move |_| store.read().level());
    let prof_bonus = Memo::new(move |_| store.read().proficiency_bonus());

    let classes = store.identity().classes();

    let add_class = move |_| {
        classes.write().push(ClassLevel::default());
    };

    let on_export = move |_| {
        store.with_untracked(export_character);
    };

    let on_import = move |_| {
        import_character(store);
    };

    let share_copied = RwSignal::new(false);

    let on_share = move |_| {
        let encoded = share::encode_character(&store.get_untracked(), Some(&registry));
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

    let on_copy = move |_| {
        let mut character = store.get();
        character.id = Uuid::new_v4();
        character.identity.name = format!("{} (Copy)", character.identity.name);
        storage::save_character(&mut character);
        let id = character.id;
        let navigate = use_navigate();
        navigate(&format!("/c/{id}"), Default::default());
    };

    let on_refill = move |_| {
        store.update(|c| {
            c.clear_all_labels();
            registry.fill_from_registry(c);
        });
    };

    let i18n = expect_context::<leptos_fluent::I18n>();

    let char_id = store.read_untracked().id;

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

                        let (race_options, race_display) = registry.with_race_entries(|entries| {
                            let options: Vec<(String, String, String)> = entries.iter().map(|entry| {
                                (entry.name.clone(), entry.label().to_string(), entry.description.clone())
                            }).collect();
                            let display = entries.iter()
                                .find(|e| e.name == race_name)
                                .map(|e| e.label().to_string())
                                .unwrap_or_else(|| race_name.clone());
                            (options, display)
                        });

                        if !race_name.is_empty() {
                            registry.fetch_race(&race_name);
                        }

                        let show_apply = registry.has_race(&race_name) && !race_applied;

                        view! {
                            <div class="race-input-row">
                                <DatalistInput
                                    value=race_display
                                    placeholder=tr!("race")
                                    options=race_options
                                    on_input=move |input, resolved| {
                                        let name: String = resolved.unwrap_or(input);
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
                                                registry.with_race(&race_name, |def| {
                                                    store.update(|c| def.apply(c));
                                                });
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

                        let (bg_options, bg_display) = registry.with_background_entries(|entries| {
                            let options: Vec<(String, String, String)> = entries.iter().map(|entry| {
                                (entry.name.clone(), entry.label().to_string(), entry.description.clone())
                            }).collect();
                            let display = entries.iter()
                                .find(|e| e.name == bg_name)
                                .map(|e| e.label().to_string())
                                .unwrap_or_else(|| bg_name.clone());
                            (options, display)
                        });

                        if !bg_name.is_empty() {
                            registry.fetch_background(&bg_name);
                        }

                        let show_apply = registry.has_background(&bg_name) && !bg_applied;

                        view! {
                            <div class="race-input-row">
                                <DatalistInput
                                    value=bg_display
                                    placeholder=tr!("background")
                                    options=bg_options
                                    on_input=move |input, resolved| {
                                        let name: String = resolved.unwrap_or(input);
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
                                                registry.with_background(&bg_name, |def| {
                                                    store.update(|c| def.apply(c));
                                                });
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
                            let value = event_target_value(&e);
                            if let Ok(alignment) = serde_json::from_str::<Alignment>(&value) {
                                store.identity().alignment().set(alignment);
                            }
                        }
                    >
                        {Alignment::iter()
                            .map(|alignment| {
                                let tr_key = alignment.tr_key();
                                let val = (alignment as u8).to_string();
                                let selected = move || store.identity().alignment().get() == alignment;
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
                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                store.identity().experience_points().set(value);
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
                    {
                        // Memoize class options — only recomputes when abilities change,
                        // not on every class field edit.
                        let class_options = Memo::new(move |_| {
                            let abilities = store.abilities().get();
                            registry.with_class_entries(|entries| {
                                entries.iter().filter(|entry| {
                                    entry.prerequisites.iter().all(|&ability| abilities.get(ability) >= 13)
                                }).map(|entry| {
                                    (entry.name.clone(), entry.label().to_string(), entry.description.clone())
                                }).collect::<Vec<_>>()
                            })
                        });
                    move || {
                        let class_options = class_options.get();

                        classes
                            .read()
                            .iter()
                            .enumerate()
                            .map(|(i, cl)| {
                                let class_key = cl.class.clone();
                                let subclass_key = cl.subclass.clone().unwrap_or_default();
                                let level_val = cl.level.to_string();
                                let hit_die_val = cl.hit_die_sides.to_string();
                                let current_level = cl.level;
                                let applied = cl.applied_levels.clone();
                                let class_options = class_options.clone();

                                let class_name = cl.class_label().to_string();
                                let subclass_name = cl.subclass_label()
                                    .unwrap_or(&subclass_key)
                                    .to_string();

                                // Trigger lazy fetch if definition not yet loaded
                                if !class_key.is_empty() {
                                    registry.fetch_class(&class_key);
                                }

                                let class_loaded = registry.has_class(&class_key);

                                let next_unapplied: Option<u32> = if class_loaded {
                                    (1..=current_level)
                                        .find(|lvl| !applied.contains(lvl))
                                } else {
                                    None
                                };

                                let subclass_options: Vec<(String, String, String)> = registry.with_class(&class_key, |def| {
                                    def.subclasses
                                        .values()
                                        .filter(|sc| sc.min_level() <= current_level)
                                        .map(|sc| (sc.name.clone(), sc.label().to_string(), sc.description.clone()))
                                        .collect()
                                }).unwrap_or_default();
                                let has_subclasses = !subclass_options.is_empty();

                                view! {
                                    <div class="class-entry">
                                        <DatalistInput
                                            value=class_name
                                            placeholder=tr!("class")
                                            class="class-name"
                                            options=class_options
                                            on_input=move |input, resolved| {
                                                let name: String = resolved.unwrap_or(input);
                                                classes.write()[i].class.clone_from(&name);
                                                if registry.with_class_entries(|entries| entries.iter().any(|e| e.name == name)) {
                                                    registry.fetch_class(&name);
                                                    if let Some(hit_die) = registry.with_class(&name, |def| def.hit_die) {
                                                        classes.write()[i].hit_die_sides = hit_die;
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
                                                    on_input=move |input, resolved| {
                                                        if input.is_empty() {
                                                            classes.write()[i].subclass = None;
                                                        } else {
                                                            let name: String = resolved.unwrap_or(input);
                                                            classes.write()[i].subclass = Some(name);
                                                        }
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
                                                if let Ok(value) = event_target_value(&e).parse::<u16>() {
                                                    classes.write()[i].hit_die_sides = value;
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
                                                if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                    classes.write()[i].level = value.clamp(1, 20);
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
                <button class="btn-add" title=move_tr!("share-link") on:click=on_share>
                    {move || if share_copied.get() { "\u{2705}" } else { "\u{1F4E4}" }}
                </button>
                <button class="btn-add" title=move_tr!("export-json") on:click=on_export>"\u{1F4BE}"</button>
                <button class="btn-add" title=move_tr!("import-json") on:click=on_import>"\u{1F4C2}"</button>
                <button class="btn-add" title=move_tr!("copy-character") on:click=on_copy>"\u{1F4CB}"</button>
                <button class="btn-add" title=move_tr!("refill-from-registry") on:click=on_refill>"\u{1F503}"</button>
                <button
                    class="btn-add btn-danger"
                    title=move_tr!("reset-character")
                    on:click=move |_| {
                        let msg = tr!("confirm-reset");
                        let window = leptos::prelude::window();
                        if window.confirm_with_message(&msg).unwrap_or(false) {
                            let id = store.read_untracked().id;
                            store.set(Character { id, ..Default::default() });
                        }
                    }
                >
                    "\u{1F504}"
                </button>
            </div>
            <hr />
            <div class="nav-links">
                <A href=format!("{BASE_URL}/") attr:class="back-link">{move_tr!("back-to-characters")}</A>
                <A href=format!("{BASE_URL}/c/{char_id}/summary") attr:class="back-link">
                    {move_tr!("view-summary")}
                </A>
            </div>
        </div>
    }
}
