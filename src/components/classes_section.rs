use leptos::{prelude::*, reactive::wrappers::read::ArcSignal};
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{
        character_header::split_resolved,
        datalist::{DatalistInput, DatalistOption, SharedDatalist, next_datalist_id},
        icon::Icon,
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

    // Locale-independent slice of `classes`: just `(class_name, level)`
    // tuples. The body still subscribes to the whole `classes` Vec, but the
    // Memo's `PartialEq` filters out label-only writes (from the
    // `fill_from_registry` Effect on locale switch) — downstream Memos
    // observing `class_specs.get()` don't re-evaluate when only labels
    // changed, so their `Signal::derive`-owned scopes survive.
    let class_specs: Memo<Vec<(String, u32)>> = Memo::new(move |_| {
        classes
            .read()
            .iter()
            .map(|cl| (cl.class.clone(), cl.level))
            .collect()
    });
    // Subclass options per class slot. Subscribes only to `class_specs`,
    // not to the raw `classes` Vec — so locale-induced label syncs don't
    // re-evaluate this Memo and the per-row `Signal::derive`s outlive the
    // store mutation, keeping the singleton modal's references valid.
    let subclass_options_per_class: Memo<Vec<Vec<DatalistOption>>> = Memo::new(move |_| {
        class_specs
            .read()
            .iter()
            .map(|(class_key, level)| {
                let class_key = class_key.clone();
                let level = *level;
                // Tracked-on-data only: subscribes to class definition arrival
                // but NOT to its locale overlay, so locale switches don't fire
                // this Memo (which would dispose the inner Signal::derive's).
                let names = registry
                    .classes()
                    .with(&class_key, |def| {
                        def.subclasses
                            .values()
                            .filter(|sub| sub.min_level() <= level)
                            .map(|sub| sub.name.to_string())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                names
                    .into_iter()
                    .map(|sub_name| {
                        let label = ArcSignal::derive({
                            let class_key = class_key.clone();
                            let sub_name = sub_name.clone();
                            move || {
                                registry
                                    .classes()
                                    .lookup(&class_key, |loc| {
                                        loc.subclass(&sub_name).map(|sub| sub.label().to_string())
                                    })
                                    .flatten()
                                    .unwrap_or_else(|| sub_name.clone())
                            }
                        });
                        let description = ArcSignal::derive({
                            let class_key = class_key.clone();
                            let sub_name = sub_name.clone();
                            move || {
                                registry
                                    .classes()
                                    .lookup(&class_key, |loc| {
                                        loc.subclass(&sub_name)
                                            .map(|sub| sub.description().to_string())
                                    })
                                    .flatten()
                                    .unwrap_or_default()
                            }
                        });
                        DatalistOption::with_signals(sub_name, label, description)
                    })
                    .collect()
            })
            .collect()
    });

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
                            let subclass_name = cl
                                .subclass_label()
                                .unwrap_or(&subclass_key)
                                .to_string();

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

                            // Pull this slot's subclass list from the
                            // component-scoped Memo (signals there outlive
                            // iteration disposal).
                            let subclass_options: Vec<DatalistOption> =
                                subclass_options_per_class
                                    .with(|all| all.get(i).cloned().unwrap_or_default());
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

                            let class_list_id = next_datalist_id();
                            let subclass_list_id = next_datalist_id();
                            let subclass_opts_signal = Signal::derive({
                                let options = subclass_options.clone();
                                move || options.clone()
                            });
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
                                            let hit_die =
                                                registry.classes().with_untracked(&name, |def| def.hit_die);
                                            {
                                                let mut classes = classes.write();
                                                classes[i].class.clone_from(&name);
                                                classes[i].class_label = label;
                                                if let Some(hd) = hit_die {
                                                    classes[i].hit_die_sides = hd;
                                                }
                                            }
                                            registry.classes().fetch_untracked(&name);
                                        }
                                    />
                                    {if has_subclasses {
                                        Some(
                                            view! {
                                                <SharedDatalist
                                                    id=subclass_list_id.clone()
                                                    options=subclass_opts_signal
                                                />
                                                <DatalistInput
                                                    value=subclass_name
                                                    placeholder=move_tr!("subclass")
                                                    class="class-subclass"
                                                    list_id=subclass_list_id
                                                    options=subclass_opts_signal
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
                                                                    "/r/class/{class_key}/{sub_key}"
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
