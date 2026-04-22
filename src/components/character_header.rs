use std::time::Duration;

use leptos::{leptos_dom::helpers::set_timeout, prelude::*};
use leptos_fluent::move_tr;
use leptos_router::hooks::use_navigate;
use reactive_stores::Store;
use strum::IntoEnumIterator;
use uuid::Uuid;

use crate::{
    BASE_URL,
    components::{
        apply::{rebuild, replay_with_modal},
        avatar::Avatar as AvatarView,
        avatar_generate_modal::AvatarGenerateModal,
        background_field::BackgroundField,
        classes_section::ClassesSection,
        confirm_modal::ConfirmModal,
        dropdown::{Dropdown, DropdownTrigger},
        icon::Icon,
        menu_modal::{MenuItem, MenuModal},
        species_field::SpeciesField,
    },
    export::export_character,
    firebase,
    model::{
        Alignment, AppliedStoreFields, Avatar, Character, CharacterIdentityStoreFields,
        CharacterStoreFields, PersonalityStoreFields, Translatable,
    },
    rules::{DefinitionStore, RulesRegistry},
    share, storage,
};

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
    let avatar = expect_context::<RwSignal<Option<Avatar>>>();
    let name_signal = Signal::derive(move || store.identity().name().get());

    let total_level = Memo::new(move |_| store.read().level());
    let prof_bonus = Memo::new(move |_| store.read().proficiency_bonus());

    let classes = store.identity().classes();
    let show_level_up = RwSignal::new(false);
    let i18n = expect_context::<leptos_fluent::I18n>();

    let level_up_class = move |class_idx: usize| {
        classes.write()[class_idx].level += 1;
        // No auto-apply: new class level shows a pending-dot on the class row.
        // User must click Rebuild to apply identity changes.
    };

    let on_level_up = move |_| {
        let count = classes.read().len();
        if count == 1 {
            level_up_class(0);
        } else if count > 1 {
            show_level_up.set(true);
        }
    };

    let level_up_items = Memo::new(move |_| {
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
            let origin = window().location().origin().unwrap_or_default();

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

            crate::export::copy_to_clipboard(&url);
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
    let show_avatar_generate = RwSignal::new(false);

    view! {
        <div class="panel character-header">
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
            <div class="header-layout">
                <AvatarView
                    name=name_signal
                    avatar=Signal::derive(move || avatar.get())
                    char_id=store.get_untracked().id
                    size=80
                    editable=true
                    on_change=Callback::new(move |new_avatar| avatar.set(Some(new_avatar)))
                    on_remove=Callback::new(move |_| avatar.set(None))
                    on_generate=Callback::new(move |_| show_avatar_generate.set(true))
                />
                <div class="header-content">
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
                            <label>
                                {move_tr!("species")}
                                <Show when=move || {
                                    let species = store.identity().species().get();
                                    registry.species().has_tracked(&species)
                                        && !store.applied().species().get()
                                }>
                                    <span class="pending-dot" />
                                </Show>
                            </label>
                            <div class="entity-input-row">
                                <SpeciesField />
                            </div>
                        </div>
                        <div class="header-field background-field">
                            <label>
                                {move_tr!("background")}
                                <Show when=move || {
                                    let background = store.identity().background().get();
                                    registry.backgrounds().has_tracked(&background)
                                        && !store.applied().background().get()
                                }>
                                    <span class="pending-dot" />
                                </Show>
                            </label>
                            <div class="entity-input-row">
                                <BackgroundField />
                            </div>
                        </div>
                        <div class="header-field">
                            <label>{move_tr!("alignment")}</label>
                            <select
                                on:change=move |e| {
                                    let value = event_target_value(&e);
                                    if let Some(alignment) = Alignment::from_u8_str(&value) {
                                        store.personality().alignment().set(alignment);
                                    }
                                }
                            >
                                {Alignment::iter()
                                    .map(|alignment| {
                                        let tr_key = alignment.tr_key();
                                        let val = (alignment as u8).to_string();
                                        let selected = move || {
                                            store.personality().alignment().get() == alignment
                                        };
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
                </div>
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
            <AvatarGenerateModal
                show=show_avatar_generate
                char_id=store.get_untracked().id
                on_result=Callback::new(move |new_avatar| avatar.set(Some(new_avatar)))
            />
        </div>
    }
}
