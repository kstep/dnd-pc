use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{
        apply::apply_with_modal,
        datalist::{DatalistInput, DatalistOption, SharedDatalist, next_datalist_id},
        icon::Icon,
    },
    model::{Character, Feature, FeatureSource},
    rules::{RulesRegistry, apply::PendingFeature},
};

#[component]
pub fn AddFeatureRow(options: Memo<Vec<DatalistOption>>) -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let input_value = RwSignal::new(String::new());
    let resolved_key: RwSignal<Option<String>> = RwSignal::new(None);
    let list_id = next_datalist_id();
    let list_id_for_shared = list_id.clone();
    let list_id_for_input = list_id;

    let commit = move || {
        let typed = input_value.get_untracked();
        let typed = typed.trim();
        if typed.is_empty() {
            return;
        }
        let level = store.read_untracked().level();
        let source = FeatureSource::User(level);

        let Some(key) = resolved_key.get_untracked() else {
            // Free-text custom feat — no registry binding, no apply pipeline.
            let label = typed.to_string();
            store.update(|character| {
                character.features.list.push(Feature {
                    name: String::new(),
                    label: Some(label),
                    applied: true,
                    source,
                    ..Feature::default()
                });
            });
            input_value.set(String::new());
            resolved_key.set(None);
            return;
        };

        let feat_def = registry.with_features_index_untracked(|features_index| {
            features_index.get(key.as_str()).cloned()
        });
        let Some(feat_def) = feat_def else {
            log::warn!("AddFeatureRow: registry key disappeared between pick and commit: {key}");
            input_value.set(String::new());
            resolved_key.set(None);
            return;
        };

        let already_applied = store.with_untracked(|character| {
            character
                .features
                .contains(&feat_def.name, feat_def.stackable, &source)
        });
        if already_applied {
            log::debug!("AddFeatureRow: skipping duplicate non-stackable feat: {key}");
            input_value.set(String::new());
            resolved_key.set(None);
            return;
        }

        let (label, description) = registry
            .features()
            .lookup_untracked(key.as_str(), |loc| {
                let label = loc
                    .locale
                    .and_then(|map| map.get(&*loc.data.name))
                    .and_then(|text| text.label.clone());
                (label, loc.description().to_string())
            })
            .unwrap_or_default();

        let category = feat_def.category;
        let pushed_source = source.clone();
        store.update(|character| {
            character.features.list.push(Feature {
                name: key.clone(),
                label,
                description,
                applied: false,
                category,
                source: pushed_source,
                ..Feature::default()
            });
        });

        let pending = vec![PendingFeature {
            name: key,
            source,
            level,
            replaces: None,
        }];
        apply_with_modal(store, registry, pending, None, None, |_| {});

        input_value.set(String::new());
        resolved_key.set(None);
    };

    view! {
        <form
            class="entry-item entry-item-add"
            on:submit=move |event| {
                event.prevent_default();
                commit();
            }
        >
            <div class="entry-content">
                <SharedDatalist id=list_id_for_shared.clone() options=options />
                <DatalistInput
                    value=input_value
                    placeholder=move_tr!("btn-add-feature")
                    class="entry-name"
                    list_id=list_id_for_input.clone()
                    options=options
                    on_input=move |input, resolved| {
                        input_value.set(input);
                        resolved_key.set(resolved);
                    }
                />
            </div>
            <div class="entry-actions">
                <button
                    type="submit"
                    class="btn-add-feature"
                    disabled=move || input_value.with(String::is_empty)
                    title=move_tr!("btn-add-feature")
                >
                    <Icon name="plus" />
                </button>
            </div>
        </form>
    }
}
