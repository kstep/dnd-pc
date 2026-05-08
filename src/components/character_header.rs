use std::time::Duration;

use leptos::{either::Either, leptos_dom::helpers::set_timeout, prelude::*};
use leptos_fluent::move_tr;
use leptos_router::hooks::use_navigate;
use reactive_stores::Store;
use uuid::Uuid;

use crate::{
    BASE_URL,
    components::{
        apply::{level_up_class, rebuild},
        avatar::Avatar as AvatarView,
        avatar_generate_modal::AvatarGenerateModal,
        confirm_modal::ConfirmModal,
        dropdown::{Dropdown, DropdownTrigger},
        icon::Icon,
        ref_link::Ref,
    },
    export::export_character,
    firebase,
    model::{
        Avatar, Character, CharacterCoreStoreFields, CharacterIdentityStoreFields,
        CharacterStoreFields, PersonalityStoreFields,
    },
    rules::{IndexEntry, RulesRegistry},
    storage,
};

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
    let name_signal = Signal::derive(move || store.personality().name().get());

    let total_level = Memo::new(move |_| store.read().level());
    let prof_bonus = Memo::new(move |_| store.read().proficiency_bonus());
    let classes = store.core().identity().classes();

    let on_level_up = move |_| {
        let prefilled_class = store
            .read_untracked()
            .identity
            .classes
            .iter()
            .find(|class_level| !class_level.class.is_empty())
            .map(|class_level| class_level.class.clone());
        level_up_class(store, registry, prefilled_class);
    };

    let on_export = move |_| {
        store.with_untracked(export_character);
    };

    let on_import = move |_| {
        import_character(store);
    };

    let share_copied = RwSignal::new(false);

    let can_share = Memo::new(move |_| store.shared().get() && firebase::current_uid().is_some());

    let on_share = move || {
        let character = store.get_untracked();
        let Some(uid) = firebase::current_uid() else {
            return;
        };
        let origin = window().location().origin().unwrap_or_default();
        let url = format!("{origin}{BASE_URL}/s/{uid}/{}", character.id);
        crate::export::copy_to_clipboard(&url);
        share_copied.set(true);
        set_timeout(move || share_copied.set(false), Duration::from_secs(2));
    };

    let on_copy = move |_| {
        let mut character = store.get_untracked();
        character.id = Uuid::new_v4();
        character.personality.name = format!("{} (Copy)", character.personality.name);
        storage::save_and_sync_character(&mut character);
        let id = character.id;
        let navigate = use_navigate();
        navigate(&format!("/c/{id}"), Default::default());
    };

    let show_rebuild_confirm = RwSignal::new(false);
    let show_reset_confirm = RwSignal::new(false);
    let show_avatar_generate = RwSignal::new(false);
    let show_remove_class = RwSignal::new(false);
    let remove_class_target: RwSignal<Option<String>> = RwSignal::new(None);

    let confirm_remove_class = move || {
        let Some(class_name) = remove_class_target.get_untracked() else {
            return;
        };
        store
            .core()
            .identity()
            .classes()
            .write()
            .retain(|cl| cl.class != class_name);
        remove_class_target.set(None);
    };

    let multiclass = Memo::new(move |_| {
        classes
            .read()
            .iter()
            .filter(|cl| !cl.class.is_empty())
            .count()
            > 1
    });

    view! {
        <div class="panel character-header">
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
                    <div class="header-name-row">
                        <input
                            class="header-name-input"
                            type="text"
                            prop:value=move || store.personality().name().get()
                            on:change=move |event| {
                                store.personality().name().set(event_target_value(&event));
                            }
                        />
                        <div class="header-actions">
                            <Show when=move || registry.can_level_up(&store.read())>
                                <button
                                    class="btn-primary btn-level-up"
                                    title=move_tr!("level-up")
                                    on:click=on_level_up
                                >
                                    <Icon name="arrow-up" />
                                    <span class="btn-level-up-label">
                                        " "
                                        {move_tr!("level-up")}
                                    </span>
                                </button>
                            </Show>
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
                                        on:change=move |event| store.shared().set(event_target_checked(&event))
                                    />
                                    <span class="dropdown-item-label">{move_tr!("share-toggle")}</span>
                                </label>
                                <button
                                    class="dropdown-item"
                                    prop:disabled=move || !can_share.get()
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
                    </div>

                    <div class="header-class-tags">
                        {move || classes
                            .read()
                            .iter()
                            .filter(|cl| !cl.class.is_empty())
                            .map(|cl| {
                                let class_key = cl.class.clone();
                                let class_label = cl.class_label().to_string();
                                let subclass_key = cl.subclass.clone().unwrap_or_default();
                                let subclass_label = cl
                                    .subclass_label()
                                    .map(str::to_string)
                                    .or_else(|| cl.subclass.clone());
                                let level = cl.level;
                                let subclass_view = subclass_label.map(|label| view! {
                                    <Ref
                                        href=format!("/r/class/{class_key}/{subclass_key}")
                                        attr:class="class-tag-subclass"
                                    >
                                        {label}
                                    </Ref>
                                });
                                let remove_target = class_key.clone();
                                view! {
                                    <span class="class-tag">
                                        <Ref
                                            href=format!("/r/class/{class_key}")
                                            attr:class="class-tag-name"
                                        >
                                            {class_label}
                                        </Ref>
                                        {subclass_view}
                                        <span class="class-tag-level">{level}</span>
                                        <Show when=move || multiclass.get()>
                                            <button
                                                class="class-tag-remove"
                                                title=move_tr!("remove-class")
                                                on:click={
                                                    let target = remove_target.clone();
                                                    move |_| {
                                                        remove_class_target.set(Some(target.clone()));
                                                        show_remove_class.set(true);
                                                    }
                                                }
                                            >
                                                <Icon name="x" size=12 />
                                            </button>
                                        </Show>
                                    </span>
                                }
                            })
                            .collect_view()
                        }
                    </div>

                    <div class="header-stats-row">
                        <div class="header-stat">
                            <span class="header-stat-label">{move_tr!("species")}</span>
                            <IdentitySlotDisplay
                                name=Signal::derive(move || store.core().identity().species().get())
                                kind=|species_name: &str| IndexEntry::Species(species_name)
                                ref_prefix="species"
                            />
                        </div>
                        <div class="header-stat">
                            <span class="header-stat-label">{move_tr!("background")}</span>
                            <IdentitySlotDisplay
                                name=Signal::derive(move || store.core().identity().background().get())
                                kind=|background_name: &str| IndexEntry::Background(background_name)
                                ref_prefix="background"
                            />
                        </div>
                        <div class="header-stat">
                            <span class="header-stat-label">{move_tr!("total-level")}</span>
                            <span class="header-stat-value">{total_level}</span>
                        </div>
                        <div class="header-stat">
                            <span class="header-stat-label">{move_tr!("prof-bonus")}</span>
                            <span class="header-stat-value">"+" {prof_bonus}</span>
                        </div>
                        <div class="header-stat">
                            <span class="header-stat-label">{move_tr!("xp")}</span>
                            <input
                                class="header-stat-input"
                                type="number"
                                min="0"
                                prop:value=move || store.core().identity().experience_points().get()
                                on:change=move |event| {
                                    if let Ok(value) = event_target_value(&event).parse::<u32>() {
                                        store.core().identity().experience_points().set(value);
                                    }
                                }
                            />
                        </div>
                    </div>
                </div>
            </div>
            <ConfirmModal
                show=show_rebuild_confirm
                title=move_tr!("rebuild")
                message=move_tr!("rebuild-confirm")
                on_confirm=move || rebuild(store, registry)
            />
            <ConfirmModal
                show=show_reset_confirm
                title=move_tr!("reset-character")
                message=move_tr!("confirm-reset")
                on_confirm=move || store.update(|character| character.clear())
            />
            <ConfirmModal
                show=show_remove_class
                title=move_tr!("remove-class")
                message=move_tr!("confirm-remove-class")
                on_confirm=confirm_remove_class
            />
            <AvatarGenerateModal
                show=show_avatar_generate
                char_id=store.get_untracked().id
                on_result=Callback::new(move |new_avatar| avatar.set(Some(new_avatar)))
            />
        </div>
    }
}

/// Read-only header display for an identity slot (Species / Background).
/// Renders the locale-resolved label as a link to the reference page when
/// `name` is non-empty, or an em-dash placeholder otherwise. Edits flow
/// through the cascade modal (Class Level / Subclass picker), not inline.
#[component]
fn IdentitySlotDisplay<K>(name: Signal<String>, kind: K, ref_prefix: &'static str) -> impl IntoView
where
    K: for<'a> Fn(&'a str) -> IndexEntry<'a> + Copy + Send + Sync + 'static,
{
    let registry = expect_context::<RulesRegistry>();
    let label = Signal::derive(move || {
        let current = name.read();
        if current.is_empty() {
            return String::new();
        }
        let (label, _) = registry.index().entry_label_desc(kind(&current));
        label.get()
    });
    view! {
        {move || {
            let current = name.read();
            if current.is_empty() {
                Either::Left(view! { <span class="identity-slot-empty">"—"</span> })
            } else {
                Either::Right(view! {
                    <Ref
                        href=format!("/r/{ref_prefix}/{}", *current)
                        attr:class="identity-slot-link"
                    >
                        {label}
                    </Ref>
                })
            }
        }}
    }
}
