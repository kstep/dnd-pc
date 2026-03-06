use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::summary_list::{SummaryList, SummaryListItem},
    model::{Character, CharacterIdentity, CharacterStoreFields, FeatureValue},
    rules::{ChoiceOptions, FieldKind, RulesRegistry},
};

fn is_choice_ref(
    registry: &RulesRegistry,
    identity: &CharacterIdentity,
    feat_name: &str,
    field_name: &str,
) -> bool {
    registry
        .with_feature(identity, feat_name, |feat| {
            feat.fields.get(field_name).is_some_and(|fd| {
                matches!(&fd.kind, FieldKind::Choice { options, .. } if matches!(options, ChoiceOptions::Ref { .. }))
            })
        })
        .unwrap_or(false)
}

#[component]
pub fn ChoicesBlock() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let store = expect_context::<Store<Character>>();

    let identity = store.identity();
    let choices = store
        .feature_data()
        .read()
        .iter()
        .flat_map(|(feat_name, entry)| {
            entry.fields.iter().filter_map(|field| {
                if let FeatureValue::Choice { options } = &field.value
                    && !is_choice_ref(&registry, &identity.read(), feat_name, &field.name)
                {
                    let selected = options
                        .iter()
                        .map(|opt| (opt.label().to_string(), opt.cost, opt.description.clone()))
                        .collect::<Vec<_>>();
                    if selected.is_empty() {
                        return None;
                    }
                    Some((field.label().to_string(), selected))
                } else {
                    None
                }
            })
        })
        .collect::<Vec<_>>();

    if choices.is_empty() {
        return None;
    }

    Some(
        choices
            .into_iter()
            .map(|(label, options)| {
                view! {
                    <h4 class="summary-subsection-title">{label}</h4>
                    <SummaryList items={options.into_iter().map(|(name, cost, description)| {
                        SummaryListItem {
                            name,
                            description,
                            badge: (cost > 0).then(|| view! {
                                <span class="summary-choice-cost">{cost}</span>
                            }.into_any()),
                        }
                    }).collect::<Vec<_>>()} />
                }
            })
            .collect_view(),
    )
}
