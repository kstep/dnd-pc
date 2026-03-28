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
        background_field::BackgroundField,
        classes_section::ClassesSection,
        icon::Icon,
        menu_modal::{MenuItem, MenuModal},
        species_field::SpeciesField,
    },
    firebase,
    model::{
        Alignment, Character, CharacterIdentityStoreFields, CharacterStoreFields, Translatable,
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
        "character.dnd.json".to_string()
    } else {
        format!("{}.dnd.json", character.identity.name)
    };

    anchor.set_href(&url);
    anchor.set_download(&filename);
    anchor.click();

    let _ = web_sys::Url::revoke_object_url(&url);
}

pub(crate) fn split_resolved(input: String, resolved: Option<String>) -> (String, Option<String>) {
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

pub(crate) fn apply_with_args_modal(
    pending: Vec<crate::rules::PendingArgs>,
    apply: impl Fn(Option<&BTreeMap<String, Vec<Vec<i32>>>>) + Copy + Send + Sync + 'static,
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
            registry.apply_class_level(c, class_index, level, args_map);
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
                    <SpeciesField />
                </div>
                <div class="header-field background-field">
                    <label>{move_tr!("background")}</label>
                    <BackgroundField />
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

            <ClassesSection />

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
                <button class="btn-primary" title=move_tr!("share-link") on:click=on_share>
                    <Icon name=move || if share_copied.get() { "check" } else { "share-2" } size=18 />
                </button>
                <button class="btn-primary" title=move_tr!("export-json") on:click=on_export><Icon name="download" size=18 /></button>
                <button class="btn-primary" title=move_tr!("import-json") on:click=on_import><Icon name="upload" size=18 /></button>
                <button class="btn-primary" title=move_tr!("copy-character") on:click=on_copy><Icon name="copy" size=18 /></button>
                <button class="btn-primary" title=move_tr!("refill-from-registry") on:click=on_refill><Icon name="book-up" size=18 /></button>
                <button
                    class="btn-primary btn-danger"
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
