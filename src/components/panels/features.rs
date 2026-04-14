use leptos::prelude::*;
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;

use crate::{
    components::feature_row::FeatureRow,
    model::{Character, CharacterStoreFields, Feature, FeatureSource},
    rules::RulesRegistry,
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

    view! {
        <button class="btn-primary" on:click=add_feature>
            {move_tr!("btn-add-feature")}
        </button>
        <div class="entry-list">
            {move || {
                let features_read = features.read();
                features_read
                    .iter()
                    .enumerate()
                    .rev()
                    .map(|(i, feature)| {
                        let is_group_boundary = i == 0
                            || features_read[i - 1].source != feature.source;
                        let header = is_group_boundary
                            .then(|| {
                                let label = registry.source_label(&feature.source, i18n);
                                view! { <h3 class="features-group-header">{label}</h3> }
                            });
                        view! {
                            {header}
                            <FeatureRow feature_idx=i options=feature_options />
                        }
                    })
                    .collect_view()
            }}
        </div>
    }
}
