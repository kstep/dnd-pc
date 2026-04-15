use std::time::Duration;

use leptos::{leptos_dom::helpers::set_timeout, prelude::*};
use leptos_fluent::move_tr;
use leptos_router::hooks::use_navigate;
use reactive_stores::Store;
use strum::IntoEnumIterator;
use uuid::Uuid;
use wasm_bindgen::prelude::*;

use crate::{
    BASE_URL,
    components::{
        apply::{apply_level, apply_with_modal, rebuild, replay_with_modal},
        apply_field_section::ApplyFieldSection,
        background_field::BackgroundField,
        classes_section::ClassesSection,
        confirm_modal::ConfirmModal,
        dropdown::{Dropdown, DropdownTrigger},
        icon::Icon,
        menu_modal::{MenuItem, MenuModal},
        species_field::SpeciesField,
    },
    firebase,
    model::{
        Alignment, Character, CharacterIdentityStoreFields, CharacterStoreFields, Translatable,
    },
    rules::{
        DefinitionStore, RulesRegistry,
        apply::{apply_new_features, collect_background_features, collect_species_features},
    },
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

pub fn split_resolved(input: String, resolved: Option<String>) -> (String, Option<String>) {
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
        classes.write()[class_idx].level += 1;
        apply_level(store, registry);
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

    let on_export = move |_| {
        store.with_untracked(export_character);
    };

    let on_import = move |_| {
        import_character(store);
    };

    let share_copied = RwSignal::new(false);

    let on_share = move || {
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

    let show_replay_confirm = RwSignal::new(false);
    let show_rebuild_confirm = RwSignal::new(false);
    let show_reset_confirm = RwSignal::new(false);

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
                <ApplyFieldSection
                    label=move_tr!("species")
                    class="species-field"
                    applied=move || store.identity().species_applied().get()
                    ready=move || {
                        let species = store.identity().species().get();
                        registry.species().has_tracked(&species)
                    }
                    apply_title=move_tr!("btn-apply-species")
                    on_apply=move || {
                        let pending = store.with_untracked(|character| {
                            let species_cache = registry.species().cache().read_untracked();
                            registry.with_features_index_untracked(|fi| {
                                species_cache
                                    .get(character.identity.species.as_str())
                                    .into_iter()
                                    .flat_map(|species_def| {
                                        collect_species_features(character, species_def, fi)
                                    })
                                    .collect()
                            })
                        });
                        apply_with_modal(
                            store,
                            registry,
                            pending,
                            move |character, pending, inputs, fi| {
                                character.identity.species_applied = true;
                                apply_new_features(fi, character, pending, Some(inputs));
                            },
                        );
                    }
                >
                    <SpeciesField />
                </ApplyFieldSection>
                <ApplyFieldSection
                    label=move_tr!("background")
                    class="background-field"
                    applied=move || store.identity().background_applied().get()
                    ready=move || {
                        let background = store.identity().background().get();
                        registry.backgrounds().has_tracked(&background)
                    }
                    apply_title=move_tr!("btn-apply-background")
                    on_apply=move || {
                        let pending = store.with_untracked(|character| {
                            let bg_cache = registry.backgrounds().cache().read_untracked();
                            registry.with_features_index_untracked(|fi| {
                                bg_cache
                                    .get(character.identity.background.as_str())
                                    .into_iter()
                                    .flat_map(|bg_def| {
                                        collect_background_features(character, bg_def, fi)
                                    })
                                    .collect()
                            })
                        });
                        apply_with_modal(
                            store,
                            registry,
                            pending,
                            move |character, pending, inputs, fi| {
                                character.identity.background_applied = true;
                                apply_new_features(fi, character, pending, Some(inputs));
                            },
                        );
                    }
                >
                    <BackgroundField />
                </ApplyFieldSection>
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
                        <span class="stat-highlight">{total_level}</span>
                        <Show when=move || store.read().can_level_up()>
                            <button
                                class="btn-level-up"
                                title=move_tr!("level-up")
                                on:click=on_level_up
                            >
                                <Icon name="arrow-up" />
                            </button>
                        </Show>
                    </div>
                </div>
                <div class="header-field level-field">
                    <label>{move_tr!("prof-bonus")}</label>
                    <span class="stat-highlight">"+" {prof_bonus}</span>
                </div>
            </div>

            <ClassesSection />

            <div class="header-actions">
                <Dropdown class="dropdown-end">
                    <DropdownTrigger slot>
                        <button class="btn-icon" title=move_tr!("actions-menu")>
                            <Icon name="ellipsis-vertical" size=18 />
                        </button>
                    </DropdownTrigger>

                    <label
                        class="dropdown-item"
                        on:click=move |ev| ev.stop_propagation()
                    >
                        <input
                            type="checkbox"
                            prop:checked=move || store.shared().get()
                            on:change=move |e| store.shared().set(event_target_checked(&e))
                        />
                        <span class="dropdown-item-label">{move_tr!("share-toggle")}</span>
                    </label>
                    <button
                        class="dropdown-item"
                        on:click=move |ev| {
                            ev.stop_propagation();
                            on_share();
                        }
                    >
                        <Icon name=move || if share_copied.get() { "check" } else { "share-2" } size=16 />
                        <span class="dropdown-item-label">{move_tr!("share-link")}</span>
                    </button>

                    <div class="dropdown-separator"></div>

                    <button class="dropdown-item" on:click=on_export>
                        <Icon name="download" size=16 />
                        <span class="dropdown-item-label">{move_tr!("export-json")}</span>
                    </button>
                    <button class="dropdown-item" on:click=on_import>
                        <Icon name="upload" size=16 />
                        <span class="dropdown-item-label">{move_tr!("import-json")}</span>
                    </button>

                    <div class="dropdown-separator"></div>

                    <button class="dropdown-item" on:click=on_copy>
                        <Icon name="copy" size=16 />
                        <span class="dropdown-item-label">{move_tr!("copy-character")}</span>
                    </button>

                    <div class="dropdown-separator"></div>

                    <button
                        class="dropdown-item"
                        on:click=move |_| show_replay_confirm.set(true)
                    >
                        <Icon name="rotate-ccw" size=16 />
                        <span class="dropdown-item-label">{move_tr!("replay")}</span>
                    </button>
                    <button
                        class="dropdown-item"
                        on:click=move |_| show_rebuild_confirm.set(true)
                    >
                        <Icon name="wrench" size=16 />
                        <span class="dropdown-item-label">{move_tr!("rebuild")}</span>
                    </button>
                    <button
                        class="dropdown-item dropdown-item-danger"
                        on:click=move |_| show_reset_confirm.set(true)
                    >
                        <Icon name="rotate-ccw" size=16 />
                        <span class="dropdown-item-label">{move_tr!("reset-character")}</span>
                    </button>
                </Dropdown>
            </div>
            <MenuModal
                show=show_level_up
                title=Signal::derive(move || i18n.tr("level-up-choose-class"))
                items=level_up_items
                on_select=Callback::new(level_up_class)
            />
            <ConfirmModal
                show=show_replay_confirm
                title=Signal::derive(move || i18n.tr("replay"))
                message=Signal::derive(move || i18n.tr("replay-confirm"))
                on_confirm=move || replay_with_modal(store, registry)
            />
            <ConfirmModal
                show=show_rebuild_confirm
                title=Signal::derive(move || i18n.tr("rebuild"))
                message=Signal::derive(move || i18n.tr("rebuild-confirm"))
                on_confirm=move || rebuild(store, registry)
            />
            <ConfirmModal
                show=show_reset_confirm
                title=Signal::derive(move || i18n.tr("reset-character"))
                message=Signal::derive(move || i18n.tr("confirm-reset"))
                on_confirm=move || store.update(|character| character.clear())
            />
        </div>
    }
}
