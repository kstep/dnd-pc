use leptos::prelude::*;
use leptos_fluent::{move_tr, tr};
use reactive_stores::Store;

use super::character_header::{apply_single_level, split_resolved};
use crate::{
    BASE_URL,
    components::{datalist_input::DatalistInput, icon::Icon},
    model::{Character, CharacterIdentityStoreFields, CharacterStoreFields, ClassLevel},
    rules::{DefinitionStore, RulesRegistry},
};

#[component]
pub fn ClassesSection() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let classes = store.identity().classes();

    let add_class = move |_| {
        classes.write().push(ClassLevel::default());
    };

    // All classes (for first class — no prerequisites).
    let all_class_options = Memo::new(move |_| {
        registry.with_class_entries(|entries| {
            entries
                .values()
                .map(|entry| {
                    (
                        entry.name.clone(),
                        entry.label().to_string(),
                        entry.description.clone(),
                    )
                })
                .collect::<Vec<_>>()
        })
    });
    // Filtered by prerequisites (for multiclassing — all classes
    // must meet their prerequisites).
    let multiclass_options = Memo::new(move |_| {
        let character = store.get();
        registry.with_class_entries(|entries| {
            entries
                .values()
                .filter(|entry| registry.can_multiclass(&character, &entry.name))
                .map(|entry| {
                    (
                        entry.name.clone(),
                        entry.label().to_string(),
                        entry.description.clone(),
                    )
                })
                .collect::<Vec<_>>()
        })
    });

    view! {
        <div class="classes-section">
            <label>{move_tr!("classes")}</label>
            <div class="classes-list">
                {move || {
                    classes
                        .read()
                        .iter()
                        .enumerate()
                        .map(|(i, cl)| {
                            let class_key = cl.class.clone();
                            let subclass_key = cl.subclass.clone().unwrap_or_default();
                            let level_val = cl.level.to_string();
                            let current_level = cl.level;
                            let class_name = cl.class_label().to_string();
                            let subclass_name = cl
                                .subclass_label()
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

                            let subclass_options: Vec<(String, String, String)> = registry
                                .classes()
                                .with(&class_key, |def| {
                                    def.subclasses
                                        .values()
                                        .filter(|sc| sc.min_level() <= current_level)
                                        .map(|sc| {
                                            (
                                                sc.name.clone(),
                                                sc.label().to_string(),
                                                sc.description.clone(),
                                            )
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            let has_subclasses = !subclass_options.is_empty();
                            let hit_die_sides = Memo::new(move |_| {
                                classes.read().get(i).map_or(8, |cl| cl.hit_die_sides)
                            });

                            let class_opts = Signal::derive(move || {
                                if classes.read().len() <= 1 {
                                    all_class_options.get()
                                } else {
                                    multiclass_options.get()
                                }
                            });

                            view! {
                                <div class="class-entry">
                                    <DatalistInput
                                        value=class_name
                                        placeholder=move_tr!("class")
                                        class="class-name"
                                        options=class_opts
                                        ref_href=move || {
                                            (!class_key.is_empty())
                                                .then(|| format!("{BASE_URL}/r/class/{class_key}"))
                                        }
                                        on_input=move |input, resolved| {
                                            let (name, label) = split_resolved(input, resolved);
                                            let hit_die =
                                                registry.classes().with(&name, |def| def.hit_die);
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
                                        Some(
                                            view! {
                                                <DatalistInput
                                                    value=subclass_name
                                                    placeholder=move_tr!("subclass")
                                                    class="class-subclass"
                                                    options=subclass_options
                                                    ref_href=move || {
                                                        let classes = classes.read();
                                                        let cl = classes.get(i)?;
                                                        let class_key = cl.class.as_str();
                                                        let sub_key = cl
                                                            .subclass
                                                            .as_deref()
                                                            .unwrap_or_default();
                                                        (!class_key.is_empty()
                                                            && !sub_key.is_empty())
                                                            .then(|| {
                                                                format!(
                                                                    "{BASE_URL}/r/class/{class_key}/{sub_key}"
                                                                )
                                                            })
                                                    }
                                                    on_input=move |input, resolved| {
                                                        let mut classes = classes.write();
                                                        if input.is_empty() {
                                                            classes[i].subclass = None;
                                                            classes[i].subclass_label = None;
                                                        } else {
                                                            let (name, label) = split_resolved(
                                                                input,
                                                                resolved,
                                                            );
                                                            classes[i].subclass = Some(name);
                                                            classes[i].subclass_label = label;
                                                        }
                                                    }
                                                />
                                            },
                                        )
                                    } else {
                                        None
                                    }}
                                    <select
                                        class="class-hit-die"
                                        prop:value=cl.hit_die_sides.to_string()
                                        on:change=move |e| {
                                            if let Ok(value) = event_target_value(&e).parse::<u32>()
                                            {
                                                classes.write()[i].hit_die_sides = value;
                                            }
                                        }
                                    >
                                        <option value="6" selected=move || {
                                            hit_die_sides.get() == 6
                                        }>"d6"</option>
                                        <option value="8" selected=move || {
                                            hit_die_sides.get() == 8
                                        }>"d8"</option>
                                        <option value="10" selected=move || {
                                            hit_die_sides.get() == 10
                                        }>"d10"</option>
                                        <option value="12" selected=move || {
                                            hit_die_sides.get() == 12
                                        }>"d12"</option>
                                    </select>
                                    <input
                                        type="number"
                                        class="class-level"
                                        min="1"
                                        max="20"
                                        prop:value=level_val
                                        on:change=move |e| {
                                            if let Ok(value) = event_target_value(&e).parse::<u32>()
                                            {
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
                                        let title = tr!("btn-apply-level", { "level" => lvl });
                                        Some(
                                            view! {
                                                <div class="apply-levels">
                                                    <button
                                                        class="btn-apply-level"
                                                        title=title
                                                        on:click=move |_| {
                                                            apply_single_level(store, registry, i, lvl);
                                                        }
                                                    >
                                                        <Icon name="arrow-up" size=14 />
                                                        {lvl}
                                                    </button>
                                                </div>
                                            },
                                        )
                                    } else {
                                        None
                                    }}
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
            <button class="btn-primary" on:click=add_class>
                {move_tr!("btn-add-class")}
            </button>
        </div>
    }
}
