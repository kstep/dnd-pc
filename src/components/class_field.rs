use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use super::character_header::split_resolved;
use crate::{
    components::entity_field::EntityField,
    model::{Character, CharacterIdentityStoreFields, CharacterStoreFields},
    rules::{DefinitionStore, RulesRegistry},
};

/// Class name selector for the first class slot. Sets `classes[0].class`,
/// auto-fills `hit_die_sides`, and triggers definition fetch.
/// No level, subclass, or apply controls.
#[component]
pub fn ClassField() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let options = Memo::new(move |_| {
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

    let classes = store.identity().classes();

    // EntityField needs name as a key, but class uses split_resolved for
    // label/name separation. We pass the display label through EntityField's
    // name resolution, but override on_input to handle the split.
    let class_key = Signal::derive(move || {
        classes
            .read()
            .first()
            .map(|cl| cl.class.clone())
            .unwrap_or_default()
    });

    // Trigger lazy fetch
    Effect::new(move || {
        let key = class_key.get();
        if !key.is_empty() {
            registry.classes().fetch(&key);
        }
    });

    view! {
        <EntityField
            name=class_key
            options=options
            ref_prefix="class"
            required=true
            placeholder=move_tr!("class")
            on_input=move |input: String| {
                // EntityField resolves label→name, but class needs split_resolved
                // for the label. Re-resolve from options.
                let resolved = options
                    .read_untracked()
                    .iter()
                    .find(|(name, label, _)| *name == input || *label == input)
                    .map(|(name, _, _)| name.clone());
                let (name, label) = split_resolved(input, resolved);
                let hit_die = registry.classes().with(&name, |def| def.hit_die);
                {
                    let mut classes = classes.write();
                    classes[0].class.clone_from(&name);
                    classes[0].class_label = label;
                    if let Some(hd) = hit_die {
                        classes[0].hit_die_sides = hd;
                    }
                }
                registry.classes().fetch(&name);
            }
        />
    }
}
