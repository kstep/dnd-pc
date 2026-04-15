use leptos::prelude::*;
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;

use crate::{
    components::{apply::PreviewContext, feature_row::FeatureRow},
    model::{Character, CharacterStoreFields, Feature, FeatureSource},
    rules::{RulesRegistry, WhenCondition},
};

#[component]
pub fn FeaturesPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();
    let i18n = expect_context::<I18n>();

    crate::hooks::use_scroll_to_hash();

    let features = store.features();

    let add_feature = move |_| {
        let level = store.read_untracked().level();
        features.write().push(Feature {
            source: FeatureSource::User(level),
            ..Feature::default()
        });
    };

    let feature_options = Memo::new(move |_| {
        let character = store.read();
        registry.with_features_index(|features_index| {
            features_index
                .values()
                .filter(|feat| feat.is_selectable() && feat.meets_prerequisites(&character))
                .map(|feat| {
                    (
                        feat.name.clone(),
                        feat.label().to_string(),
                        feat.description.clone(),
                    )
                })
                .collect::<Vec<_>>()
        })
    });

    let assign_previews = Memo::new(move |_| {
        let mut blank = Character::default();
        let features_read = store.features().read();
        features_read
            .iter()
            .map(|feature| {
                let mut entries: Vec<String> = Vec::new();
                registry.with_feature(&feature.name, |feat_def| {
                    let mut ctx = PreviewContext {
                        character: &mut blank,
                        captured: Vec::new(),
                    };
                    feat_def.assign(&mut ctx, WhenCondition::OnFeatureAdd, &feature.inputs);
                    for (attr, value) in ctx.captured {
                        entries.push(format!("{}: {value}", attr.display_name(&i18n)));
                    }
                });
                entries
            })
            .collect::<Vec<_>>()
    });

    view! {
        <button class="btn-primary" on:click=add_feature>
            {move_tr!("btn-add-feature")}
        </button>
        <div class="entry-list">
            {move || {
                let features_read = features.read();
                let mut last_source: Option<&FeatureSource> = None;
                let mut rows = Vec::with_capacity(features_read.len());
                for (i, feature) in features_read.iter().enumerate().rev() {
                    let is_new_group = last_source.is_none_or(|s| s != &feature.source);
                    last_source = Some(&feature.source);
                    let header = is_new_group
                        .then(|| {
                            let label = registry.source_label(&feature.source, i18n);
                            view! { <h3 class="features-group-header">{label}</h3> }
                        });
                    rows.push(
                        view! {
                            {header}
                            <FeatureRow
                                feature_idx=i
                                options=feature_options
                                assign_previews=assign_previews
                            />
                        },
                    );
                }
                rows.collect_view()
            }}
        </div>
    }
}
