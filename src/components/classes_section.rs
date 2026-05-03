use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{
        character_header::split_resolved,
        datalist::{DatalistInput, DatalistOption, SharedDatalist, next_datalist_id},
        icon::Icon,
        ref_link::Ref,
    },
    model::{
        Character, CharacterIdentityStoreFields, CharacterStoreFields, ClassLevel, MAX_CLASS_LEVEL,
    },
    rules::{DefinitionStore, IndexEntry, RulesRegistry},
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
                    let (label, description) = registry
                        .index()
                        .entry_label_desc(IndexEntry::Class(&entry.name));
                    DatalistOption::with_signals(&*entry.name, label, description)
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
                    let (label, description) = registry
                        .index()
                        .entry_label_desc(IndexEntry::Class(&entry.name));
                    DatalistOption::with_signals(&*entry.name, label, description)
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
                            let current_level = cl.level;
                            let class_name = cl.class_label().to_string();
                            let subclass_label = cl
                                .subclass_label()
                                .map(str::to_string)
                                .or_else(|| cl.subclass.clone());

                            // Trigger lazy fetch if definition not yet loaded
                            if !class_key.is_empty() {
                                registry.classes().fetch_untracked(&class_key);
                            }

                            let class_loaded = registry.classes().has_untracked(&class_key);

                            let has_pending_level = if class_loaded {
                                let applied = store.applied().read();
                                (1..=current_level)
                                    .any(|lvl| !applied.contains_level(&class_key, lvl))
                            } else {
                                false
                            };
                            let class_max_level = registry
                                .classes()
                                .with(&class_key, |def| def.max_level())
                                .unwrap_or(MAX_CLASS_LEVEL);

                            let class_opts = Signal::derive(move || {
                                if classes.read().len() <= 1 {
                                    all_class_options.get()
                                } else {
                                    multiclass_options.get()
                                }
                            });

                            let class_list_id = next_datalist_id();
                            let subclass_href = (!class_key.is_empty()
                                && !subclass_key.is_empty())
                                .then(|| format!("/r/class/{class_key}/{subclass_key}"));
                            view! {
                                <div class="class-entry">
                                    <SharedDatalist id=class_list_id.clone() options=class_opts />
                                    <DatalistInput
                                        value=class_name
                                        placeholder=move_tr!("class")
                                        class="class-name"
                                        list_id=class_list_id
                                        options=class_opts
                                        ref_href=move || {
                                            (!class_key.is_empty())
                                                .then(|| format!("/r/class/{class_key}"))
                                        }
                                        on_input=move |input, resolved| {
                                            let (name, label) = split_resolved(input, resolved);
                                            {
                                                let mut classes = classes.write();
                                                classes[i].class.clone_from(&name);
                                                classes[i].class_label = label;
                                                // hit_die_sides is set by the
                                                // Class Proficiencies feature's
                                                // OnFeatureAdd assign.
                                            }
                                            registry.classes().fetch_untracked(&name);
                                        }
                                    />
                                    {subclass_label.map(|label| match subclass_href {
                                        Some(href) => view! {
                                            <Ref attr:class="class-subclass-label" href=href>
                                                {label}
                                            </Ref>
                                        }.into_any(),
                                        None => view! {
                                            <span class="class-subclass-label">{label}</span>
                                        }.into_any(),
                                    })}
                                    <input
                                        type="number"
                                        class="class-level"
                                        min="1"
                                        max=class_max_level
                                        prop:value=current_level
                                        on:change=move |event| {
                                            if let Ok(value) =
                                                event_target_value(&event).parse::<u32>()
                                            {
                                                classes.write()[i].level =
                                                    value.clamp(1, class_max_level);
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
                                            <Icon name="x" />
                                        </button>
                                    </Show>
                                    {has_pending_level.then(|| view! {
                                        <span class="pending-dot" />
                                    })}
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
