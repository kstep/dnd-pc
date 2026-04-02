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
        apply_field_section::ApplyFieldSection,
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
    rules::{
        ApplyInputs, DefinitionStore, PendingInputs, ReplaceWith, RulesRegistry, WhenCondition,
        apply::{
            PendingFeature, apply_new_features, collect_background_features,
            collect_class_features, collect_pending_features, collect_species_features,
            reapply_existing, resolve_replacements,
        },
        feature::FeatureDefinition,
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

/// Top-level helper for the unified feature application pipeline.
/// Collects PendingInputs from pending features, shows the args modal if
/// needed, resolves replacements, calls the user callback, then computes.
pub fn apply_with_modal(
    store: Store<Character>,
    registry: RulesRegistry,
    pending: Vec<PendingFeature>,
    callback: impl Fn(
        &mut Character,
        &[PendingFeature],
        &ApplyInputs,
        &BTreeMap<Box<str>, FeatureDefinition>,
    ) + Send
    + Sync
    + 'static,
) {
    let all_inputs: Vec<PendingInputs> = registry.with_features_index_untracked(|fi| {
        let character = store.read_untracked();

        // OnFeatureAdd inputs for new features
        let new_inputs = pending.iter().filter_map(|pending_feature| {
            let feat_def = fi.get(pending_feature.name.as_str())?;
            pending_feature.pending_inputs(feat_def, &character)
        });

        // OnLevelUp inputs for already-applied features
        let levelup_inputs =
            character
                .features
                .iter()
                .filter(|f| f.applied)
                .filter_map(|feature| {
                    let feat_def = fi.get(feature.name.as_str())?;
                    let exprs = feat_def.interactive_exprs(WhenCondition::OnLevelUp, &character);
                    (!exprs.is_empty()).then_some(PendingInputs {
                        feature_name: feature.name.clone(),
                        feature_label: feat_def.label().to_string(),
                        feature_description: feat_def.description.clone(),
                        exprs,
                        replace_with: ReplaceWith::None,
                        source: feature.source.clone(),
                    })
                });

        new_inputs.chain(levelup_inputs).collect()
    });

    let apply = move |inputs: Option<&ApplyInputs>| {
        let empty = ApplyInputs::default();
        let inputs = inputs.unwrap_or(&empty);
        store.update(|character| {
            registry.with_features_index_untracked(|fi| {
                let resolved = resolve_replacements(&pending, &inputs.replacements, fi);
                callback(character, &resolved, inputs, fi);
            });
            registry.compute(character);
        });
    };

    if all_inputs.is_empty() {
        apply(None);
    } else {
        let ctx = expect_context::<ArgsModalCtx>();
        ctx.open(all_inputs, move |inputs| apply(Some(&inputs)));
    }
}

pub fn apply_level(store: Store<Character>, registry: RulesRegistry) {
    let pending = store.with_untracked(|character| {
        registry
            .with_features_index_untracked(|fi| collect_pending_features(character, &registry, fi))
    });

    apply_with_modal(
        store,
        registry,
        pending,
        move |character, pending, inputs, fi| {
            // Mark species/background as applied if they had pending features
            if !character.identity.species_applied && !character.identity.species.is_empty() {
                character.identity.species_applied = true;
            }
            if !character.identity.background_applied && !character.identity.background.is_empty() {
                character.identity.background_applied = true;
            }
            // Mark class levels as applied
            let class_cache = registry.classes().cache().read_untracked();
            for class_level in &mut character.identity.classes {
                for lvl in 1..=class_level.level {
                    if !class_level.applied_levels.contains(&lvl) {
                        class_level.applied_levels.insert(lvl);
                    }
                }
                if let Some(def) = class_cache.get(class_level.class.as_str()) {
                    class_level.hit_die_sides = def.hit_die;
                }
            }

            reapply_existing(fi, character);
            apply_new_features(fi, character, pending, Some(inputs));
            character.combat.hp_current = character.hp_max();

            let xp_threshold = character.xp_threshold();
            if character.identity.experience_points < xp_threshold {
                character.identity.experience_points = xp_threshold;
            }
        },
    );
}

/// Apply a single class level only (used by per-level apply buttons).
pub fn apply_single_level(
    store: Store<Character>,
    registry: RulesRegistry,
    class_index: usize,
    level: u32,
) {
    let pending = store.with_untracked(|character| {
        let class_cache = registry.classes().cache().read_untracked();
        registry.with_features_index_untracked(|fi| {
            class_cache
                .get(character.identity.classes[class_index].class.as_str())
                .into_iter()
                .flat_map(|class_def| {
                    collect_class_features(character, class_index, level, class_def, fi)
                })
                .collect()
        })
    });

    apply_with_modal(
        store,
        registry,
        pending,
        move |character, pending, inputs, fi| {
            if let Some(class_level) = character.identity.classes.get_mut(class_index) {
                class_level.applied_levels.insert(level);
                registry.classes().with(&class_level.class, |def| {
                    class_level.hit_die_sides = def.hit_die;
                });
            }
            reapply_existing(fi, character);
            apply_new_features(fi, character, pending, Some(inputs));
            character.combat.hp_current = character.hp_max();
        },
    );
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
                        <span class="computed-value">{total_level}</span>
                        <Show when=move || store.read().can_level_up()>
                            <button
                                class="btn-level-up"
                                title=move_tr!("level-up")
                                on:click=on_level_up
                            >
                                <Icon name="arrow-up" size=14 />
                            </button>
                        </Show>
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
                            store.update(|c| c.clear());
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
