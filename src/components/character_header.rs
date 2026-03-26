use std::{collections::BTreeMap, time::Duration};

use leptos::{leptos_dom::helpers::set_timeout, prelude::*};
use leptos_fluent::{move_tr, tr};
use leptos_router::hooks::use_navigate;
use reactive_stores::Store;
use strum::IntoEnumIterator;
use uuid::Uuid;
use wasm_bindgen::prelude::*;

use crate::{
    BASE_URL,
    components::{
        args_modal::ArgsModalCtx,
        datalist_input::DatalistInput,
        entity_field::EntityField,
        icon::Icon,
        menu_modal::{MenuItem, MenuModal},
    },
    firebase,
    model::{
        Alignment, Character, CharacterIdentityStoreFields, CharacterStoreFields, ClassLevel,
        Translatable,
    },
    rules::{DefinitionStore, RulesRegistry},
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
        "character.dnd.json".to_string()
    } else {
        format!("{}.dnd.json", character.identity.name)
    };

    anchor.set_href(&url);
    anchor.set_download(&filename);
    anchor.click();

    let _ = web_sys::Url::revoke_object_url(&url);
}

fn split_resolved(input: String, resolved: Option<String>) -> (String, Option<String>) {
    match resolved {
        Some(name) => (name, Some(input)),
        None => (input, None),
    }
}

fn import_character(store: Store<Character>) {
    storage::pick_character_from_file(move |mut imported| {
        let current_id = store.get_untracked().id;
        imported.id = current_id;
        store.set(imported);
    });
}

fn apply_with_args_modal(
    pending: Vec<crate::rules::PendingArgs>,
    apply: impl Fn(Option<&BTreeMap<String, Vec<i32>>>) + Copy + Send + Sync + 'static,
) {
    if pending.is_empty() {
        apply(None);
    } else {
        let ctx = expect_context::<ArgsModalCtx>();
        ctx.open(pending, move |args_map| apply(Some(&args_map)));
    }
}

pub fn apply_level(
    store: Store<Character>,
    registry: RulesRegistry,
    class_index: usize,
    level: u32,
) {
    let pending = store.with_untracked(|c| registry.features_needing_args(c, class_index, level));
    apply_with_args_modal(pending, move |args_map| {
        store.update(|c| {
            if let Some(args_map) = args_map {
                registry.apply_class_level_with_args(c, class_index, level, args_map);
            } else {
                registry.apply_class_level(c, class_index, level);
            }
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
    let show_level_up = RwSignal::new(false);
    let i18n = expect_context::<leptos_fluent::I18n>();

    let level_up_class = move |class_idx: usize| {
        let new_level = classes.read()[class_idx].level + 1;
        classes.write()[class_idx].level = new_level;
        apply_level(store, registry, class_idx, new_level);
    };

    let on_level_up = move |_| {
        let count = classes.read().len();
        if count == 1 {
            level_up_class(0);
        } else if count > 1 {
            show_level_up.set(true);
        }
    };

    let level_up_items = Signal::derive(move || {
        let level_label = i18n.tr("level");
        classes
            .read()
            .iter()
            .map(|class| {
                let mut label = class.class_label().to_string();
                if let Some(sub) = class.subclass_label() {
                    label.push_str(&format!(" ({sub})"));
                }
                MenuItem {
                    label,
                    detail: format!("{level_label} {}", class.level),
                }
            })
            .collect()
    });

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
        wasm_bindgen_futures::spawn_local(async move {
            let character = store.get_untracked();
            let origin = leptos::prelude::window()
                .location()
                .origin()
                .unwrap_or_default();

            let url = if character.shared
                && let Some(uid) = firebase::current_uid()
            {
                format!("{origin}{BASE_URL}/s/{uid}/{}", character.id)
            } else {
                let Some(encoded) = share::encode_character(&character, Some(&registry)).await
                else {
                    return;
                };
                format!("{origin}{BASE_URL}/s/{encoded}")
            };

            let clipboard = leptos::prelude::window().navigator().clipboard();
            let promise = clipboard.write_text(&url);
            let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
            share_copied.set(true);
        });
        set_timeout(move || share_copied.set(false), Duration::from_secs(2));
    };

    let on_copy = move |_| {
        let mut character = store.get_untracked();
        character.id = Uuid::new_v4();
        character.identity.name = format!("{} (Copy)", character.identity.name);
        storage::save_and_sync_character(&mut character);
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

    let species_options = Memo::new(move |_| {
        registry.with_species_entries(|entries| {
            entries
                .values()
                .map(|e| (e.name.clone(), e.label().to_string(), e.description.clone()))
                .collect::<Vec<_>>()
        })
    });

    let bg_options = Memo::new(move |_| {
        registry.with_background_entries(|entries| {
            entries
                .values()
                .map(|e| (e.name.clone(), e.label().to_string(), e.description.clone()))
                .collect::<Vec<_>>()
        })
    });

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
                <div class="header-field species-field">
                    <label>{move_tr!("species")}</label>
                    <EntityField
                        name=move || store.identity().species().get()
                        applied=move || store.identity().species_applied().get()
                        options=species_options
                        ref_prefix="species"
                        apply_title=move_tr!("btn-apply-species")
                        placeholder=move_tr!("species")
                        on_input=move |name: String| {
                            let old = store.identity().species().get_untracked();
                            store.identity().species().set(name.clone());
                            if name != old {
                                store.identity().species_applied().set(false);
                            }
                            registry.species().fetch(&name);
                        }
                        fetch=move |name: &str| registry.species().fetch(name)
                        has=move |name: &str| registry.species().has_tracked(name)
                        apply=move |_name: &str| {
                            let pending = store.with_untracked(|character| {
                                registry.species().with(
                                    &character.identity.species,
                                    |species_def| {
                                        registry.pending_args_for_features(
                                            character,
                                            species_def.features.iter().map(String::as_str),
                                        )
                                    },
                                )
                            }).unwrap_or_default();
                            apply_with_args_modal(pending, move |args_map| {
                                store.update(|character| registry.apply_species(character, args_map));
                            });
                        }
                    />
                </div>
                <div class="header-field background-field">
                    <label>{move_tr!("background")}</label>
                    <EntityField
                        name=move || store.identity().background().get()
                        applied=move || store.identity().background_applied().get()
                        options=bg_options
                        ref_prefix="background"
                        apply_title=move_tr!("btn-apply-background")
                        placeholder=move_tr!("background")
                        on_input=move |name: String| {
                            let old = store.identity().background().get_untracked();
                            store.identity().background().set(name.clone());
                            if name != old {
                                store.identity().background_applied().set(false);
                            }
                            registry.backgrounds().fetch(&name);
                        }
                        fetch=move |name: &str| registry.backgrounds().fetch(name)
                        has=move |name: &str| registry.backgrounds().has_tracked(name)
                        apply=move |_name: &str| {
                            let pending = store.with_untracked(|character| {
                                registry.backgrounds().with(
                                    &character.identity.background,
                                    |bg_def| {
                                        registry.pending_args_for_features(
                                            character,
                                            bg_def.features.iter().map(String::as_str),
                                        )
                                    },
                                )
                            }).unwrap_or_default();
                            apply_with_args_modal(pending, move |args_map| {
                                store.update(|character| registry.apply_background(character, args_map));
                            });
                        }
                    />
                </div>
                <div class="header-field">
                    <label>{move_tr!("alignment")}</label>
                    <select
                        on:change=move |e| {
                            let value = event_target_value(&e);
                            if let Some(alignment) = Alignment::from_u8_str(&value) {
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
                    <div class="level-value-row">
                        <span class="computed-value">{total_level}</span>
                        <button
                            class="btn-level-up"
                            title=move_tr!("level-up")
                            on:click=on_level_up
                        >
                            <Icon name="arrow-up" size=14 />
                        </button>
                    </div>
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
                                entries.values().filter(|entry| {
                                    entry.prerequisites.iter().all(|&ability| abilities.get(ability) >= 13)
                                }).map(|entry| {
                                    (entry.name.clone(), entry.label().to_string(), entry.description.clone())
                                }).collect::<Vec<_>>()
                            })
                        });
                    move || {
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
                                let class_name = cl.class_label().to_string();
                                let subclass_name = cl.subclass_label()
                                    .unwrap_or(&subclass_key)
                                    .to_string();

                                // Trigger lazy fetch if definition not yet loaded
                                if !class_key.is_empty() {
                                    registry.classes().fetch(&class_key);
                                }

                                let class_loaded = registry.classes().has(&class_key);

                                let next_unapplied: Option<u32> = if class_loaded {
                                    (1..=current_level)
                                        .find(|lvl| !cl.applied_levels.contains(lvl))
                                } else {
                                    None
                                };

                                let subclass_options: Vec<(String, String, String)> = registry.classes().with(&class_key, |def| {
                                    def.subclasses
                                        .values()
                                        .filter(|sc| sc.min_level() <= current_level)
                                        .map(|sc| (sc.name.clone(), sc.label().to_string(), sc.description.clone()))
                                        .collect()
                                }).unwrap_or_default();
                                let has_subclasses = !subclass_options.is_empty();
                                let hit_die_sides = Memo::new(move |_| {
                                    classes.read().get(i).map_or(8, |cl| cl.hit_die_sides)
                                });

                                view! {
                                    <div class="class-entry">
                                        <DatalistInput
                                            value=class_name
                                            placeholder=move_tr!("class")
                                            class="class-name"
                                            options=class_options
                                            ref_href=move || {
                                                (!class_key.is_empty()).then(|| format!("{BASE_URL}/r/class/{class_key}"))
                                            }
                                            on_input=move |input, resolved| {
                                                let (name, label) = split_resolved(input, resolved);
                                                let hit_die = registry.classes().with(&name, |def| def.hit_die);
                                                {
                                                    let mut classes = classes.write();
                                                    classes[i].class.clone_from(&name);
                                                    classes[i].class_label = label;
                                                    if let Some(hd) = hit_die {
                                                        classes[i].hit_die_sides = hd;
                                                    }
                                                }
                                                registry.classes().fetch(&name);
                                            }
                                        />
                                        {if has_subclasses {
                                            Some(view! {
                                                <DatalistInput
                                                    value=subclass_name
                                                    placeholder=move_tr!("subclass")
                                                    class="class-subclass"
                                                    options=subclass_options
                                                    ref_href=move || {
                                                        let classes = classes.read();
                                                        let cl = classes.get(i)?;
                                                        let class_key = cl.class.as_str();
                                                        let sub_key = cl.subclass.as_deref().unwrap_or_default();
                                                        (!class_key.is_empty() && !sub_key.is_empty())
                                                            .then(|| format!("{BASE_URL}/r/class/{class_key}/{sub_key}"))
                                                    }
                                                    on_input=move |input, resolved| {
                                                        let mut classes = classes.write();
                                                        if input.is_empty() {
                                                            classes[i].subclass = None;
                                                            classes[i].subclass_label = None;
                                                        } else {
                                                            let (name, label) = split_resolved(input, resolved);
                                                            classes[i].subclass = Some(name);
                                                            classes[i].subclass_label = label;
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
                                                if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                    classes.write()[i].hit_die_sides = value;
                                                }
                                            }
                                        >
                                            <option value="6" selected=move || hit_die_sides.get() == 6>"d6"</option>
                                            <option value="8" selected=move || hit_die_sides.get() == 8>"d8"</option>
                                            <option value="10" selected=move || hit_die_sides.get() == 10>"d10"</option>
                                            <option value="12" selected=move || hit_die_sides.get() == 12>"d12"</option>
                                        </select>
                                        <input
                                            type="number"
                                            class="class-level"
                                            min="1"
                                            max="20"
                                            prop:value=level_val
                                            on:change=move |e| {
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
                                                <Icon name="x" size=14 />
                                            </button>
                                        </Show>
                                        {if let Some(lvl) = next_unapplied {
                                            let title = tr!("btn-apply-level", {"level" => lvl});
                                            Some(view! {
                                                <div class="apply-levels">
                                                    <button
                                                        class="btn-apply-level"
                                                        title=title
                                                        on:click=move |_| {
                                                            apply_level(store, registry, i, lvl);
                                                        }
                                                    >
                                                        <Icon name="arrow-up" size=14 />
                                                        {lvl}
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
                <label class="share-toggle" title=move_tr!("share-toggle")>
                    <input
                        type="checkbox"
                        prop:checked=move || store.shared().get()
                        on:change=move |e| {
                            store.shared().set(event_target_checked(&e));
                        }
                    />
                    <Icon name="globe" size=18 />
                </label>
                <button class="btn-add" title=move_tr!("share-link") on:click=on_share>
                    <Icon name=move || if share_copied.get() { "check" } else { "share-2" } size=18 />
                </button>
                <button class="btn-add" title=move_tr!("export-json") on:click=on_export><Icon name="download" size=18 /></button>
                <button class="btn-add" title=move_tr!("import-json") on:click=on_import><Icon name="upload" size=18 /></button>
                <button class="btn-add" title=move_tr!("copy-character") on:click=on_copy><Icon name="copy" size=18 /></button>
                <button class="btn-add" title=move_tr!("refill-from-registry") on:click=on_refill><Icon name="book-up" size=18 /></button>
                <button
                    class="btn-add btn-danger"
                    title=move_tr!("reset-character")
                    on:click=move |_| {
                        let msg = tr!("confirm-reset");
                        let window = leptos::prelude::window();
                        if window.confirm_with_message(&msg).unwrap_or(false) {
                            store.update(|c| c.reset());
                        }
                    }
                >
                    <Icon name="rotate-ccw" size=18 />
                </button>
            </div>
            <MenuModal
                show=show_level_up
                title=Signal::derive(move || i18n.tr("level-up-choose-class"))
                items=level_up_items
                on_select=Callback::new(level_up_class)
            />
        </div>
    }
}
