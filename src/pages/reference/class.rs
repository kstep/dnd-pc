use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::{components::A, hooks::use_params, params::Params};

use super::{ReferenceFeaturesView, ReferenceSidebar, collect_feature_views};
use crate::{
    BASE_URL,
    hooks::use_hash_href,
    model::{format_bonus, proficiency_bonus_for_level},
    rules::{DefinitionStore, FieldKind, RulesRegistry, ValueOrExpr},
};

#[derive(Params, Clone, Debug, PartialEq, Eq)]
struct ClassRefParams {
    name: Option<String>,
    subname: Option<String>,
}

#[component]
pub fn ClassReference() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let i18n = expect_context::<leptos_fluent::I18n>();
    let params = use_params::<ClassRefParams>();

    let class_name = move || params.get().ok().and_then(|p| p.name).unwrap_or_default();
    let subclass_name = move || params.get().ok().and_then(|p| p.subname);

    Effect::new(move || {
        let name = class_name();
        if !name.is_empty() {
            registry.classes().fetch_tracked(&name);
        }
    });

    let current_label = Signal::derive(move || registry.class_label_by_name(&class_name()));
    let hash_href = use_hash_href();

    let detail = move || {
        let name = class_name();
        let subname = subclass_name();

        if name.is_empty() {
            return view! {
                <div class="reference-empty">
                    <p>{move_tr!("ref-select-class")}</p>
                </div>
            }
            .into_any();
        }

        // Track features_index at top level so we re-render when it loads
        // (nested LocalResource reads inside with_tracked don't reliably track)
        registry.with_features_index(|_| {});

        let prerequisites = registry.with_class_entries(|entries| {
            entries
                .get(name.as_str())
                .and_then(|e| e.prerequisites.as_ref().map(|expr| expr.to_string()))
                .unwrap_or_default()
        });

        registry
            .classes().with_tracked(&name, |def| {
                let subclass_def =
                    subname.as_deref().and_then(|sn| def.subclasses.get(sn));

                let class_label = def.label().to_string();
                let subclass_label = subclass_def.map(|sc| sc.label().to_string());
                let title = match &subclass_label {
                    Some(sc_label) => format!("{class_label} \u{2014} {sc_label}"),
                    None => class_label.clone(),
                };
                let description = def.description.clone();
                let hit_die = format!("d{}", def.hit_die);

                // Resolve features from the global features index
                let spells_def = registry.with_features_index(|features_index| {
                    def.feature_names(subname.as_deref())
                        .find_map(|feat_name| {
                            let feat = features_index.get(feat_name)?;
                            feat.spells.as_ref().filter(|s| !s.levels.is_empty()).cloned()
                        })
                });
                let has_spells = spells_def.is_some();
                let spell_list_name =
                    spells_def.as_ref().and_then(|sd| sd.list.ref_name().map(|s: &str| s.to_string()));
                let max_spell_level = spells_def
                    .as_ref()
                    .map(|sd| {
                        sd.levels
                            .values()
                            .filter_map(|l| l.slots.as_ref())
                            .map(|s: &Vec<u32>| s.len())
                            .max()
                            .unwrap_or(0)
                    })
                    .unwrap_or(0);

                // Field columns — collect owned data since features_index borrow is temporary
                struct FieldColumn {
                    label: String,
                    kind: FieldKind,
                }
                let field_columns: Vec<FieldColumn> = registry.with_features_index(|features_index| {
                    def.feature_names(subname.as_deref())
                        .flat_map(|feat_name| {
                            features_index.get(feat_name).into_iter().flat_map(|f| {
                                f.fields.values().filter(|fd| fd.kind.has_levels()).map(
                                    |fd| FieldColumn {
                                        label: fd.label().to_string(),
                                        kind: fd.kind.clone(),
                                    },
                                )
                            })
                        })
                        .collect()
                });

                // Build progression table rows
                let table_rows: Vec<_> = (1..=20u32)
                    .map(|level| {
                        let prof_bonus = proficiency_bonus_for_level(level);

                        let mut features: Vec<(String, String)> = Vec::new();
                        registry.with_features_index(|features_index| {
                            if let Some(rules) = def.levels.get(level as usize - 1) {
                                for feat_name in &rules.features {
                                    let label = features_index
                                        .get(feat_name.as_str())
                                        .map(|f| f.label().to_string())
                                        .unwrap_or_else(|| feat_name.clone());
                                    features.push((feat_name.clone(), label));
                                }
                            }
                            if let Some(sc) = subclass_def
                                && let Some(sc_rules) = sc.levels.get(&level)
                            {
                                for feat_name in &sc_rules.features {
                                    let label = features_index
                                        .get(feat_name.as_str())
                                        .map(|f| f.label().to_string())
                                        .unwrap_or_else(|| feat_name.clone());
                                    features.push((feat_name.clone(), label));
                                }
                            }
                        });

                        let spell_level_rules =
                            spells_def.as_ref().and_then(|sd| sd.levels.at_level(level));
                        let cantrips = spell_level_rules.and_then(|r| r.cantrips);
                        let spells_known = spell_level_rules.and_then(|r| r.spells);
                        let slots = spell_level_rules
                            .and_then(|r| r.slots.as_deref())
                            .unwrap_or_default();

                        let field_values: Vec<String> = field_columns
                            .iter()
                            .map(|fc| match &fc.kind {
                                FieldKind::Points { levels, .. }
                                | FieldKind::FreeUses { levels } => {
                                    match levels.at_level(level) {
                                        Some(v @ ValueOrExpr::Value(1..)) | Some(v @ ValueOrExpr::Expr(_)) => v.to_string(),
                                        _ => "\u{2014}".into(),
                                    }
                                }
                                FieldKind::Die { levels } => {
                                    match levels.at_level(level) {
                                        Some(de) if !matches!(de.amount, ValueOrExpr::Value(0)) => {
                                            de.to_string()
                                        }
                                        _ => "\u{2014}".into(),
                                    }
                                }
                                FieldKind::Choice { levels, .. } => {
                                    let v: u32 = levels.get_for_level(level);
                                    if v > 0 {
                                        v.to_string()
                                    } else {
                                        "\u{2014}".into()
                                    }
                                }
                                FieldKind::Bonus { levels } => {
                                    let v: i32 = levels.get_for_level(level);
                                    if v != 0 {
                                        format_bonus(v)
                                    } else {
                                        "\u{2014}".into()
                                    }
                                }
                            })
                            .collect();

                        (
                            level,
                            prof_bonus,
                            features,
                            cantrips,
                            spells_known,
                            slots,
                            field_values,
                        )
                    })
                    .collect();

                let class_features = registry.with_features_index(|features_index| {
                    let class_feat_iter = def.feature_names(None)
                        .filter_map(|name| features_index.get(name));
                    collect_feature_views(class_feat_iter)
                });

                let subclass_features = registry.with_features_index(|features_index| {
                    subclass_def
                        .map(|sc| {
                            let sc_feat_iter = sc.levels.values()
                                .flat_map(|lr| lr.features.iter())
                                .filter_map(|name| features_index.get(name.as_str()));
                            collect_feature_views(sc_feat_iter)
                        })
                        .unwrap_or_default()
                });

                let subclass_list: Vec<(String, String, String)> = if subclass_def.is_none() {
                    def.subclasses
                        .values()
                        .map(|sc| {
                            (
                                sc.name.clone(),
                                sc.label().to_string(),
                                sc.description.clone(),
                            )
                        })
                        .collect()
                } else {
                    Vec::new()
                };

                let subclass_desc = subclass_def
                    .map(|sc| sc.description.clone())
                    .unwrap_or_default();

                let name_for_link = name.clone();

                view! {
                    <Title text=title.clone() />
                    <div class="reference-detail">
                        <h1>{if let Some(sc_label) = subclass_label {
                            let class_href = format!("{BASE_URL}/r/class/{name}");
                            Either::Left(view! {
                                <A href=class_href>{class_label}</A>
                                {" \u{2014} "}
                                {sc_label}
                            })
                        } else {
                            Either::Right(class_label)
                        }}</h1>
                        <p class="reference-description">{description}</p>

                        {(!subclass_desc.is_empty()).then(|| view! {
                            <p class="reference-description">{subclass_desc}</p>
                        })}

                        <div class="reference-info-bar">
                            <div class="info-item">
                                <span class="info-label">{move_tr!("ref-hit-die")}</span>
                                <span class="info-value">{hit_die}</span>
                            </div>
                            {(!prerequisites.is_empty()).then(|| view! {
                                <div class="info-item">
                                    <span class="info-label">{move_tr!("ref-prerequisites")}</span>
                                    <span class="info-value">{prerequisites.clone()}</span>
                                </div>
                            })}
                            {spell_list_name.map(|sln| view! {
                                <div class="info-item">
                                    <span class="info-label">{move_tr!("ref-spell-list-link")}</span>
                                    <span class="info-value">
                                        <A href=format!("{BASE_URL}/r/spell/{sln}")>
                                            {move_tr!("ref-spell-list-link")}
                                        </A>
                                    </span>
                                </div>
                            })}
                        </div>

                        <h2>{move_tr!("ref-progression")}</h2>
                        <div class="progression-table-wrapper">
                            <table class="progression-table">
                                <thead>
                                    <tr>
                                        <th>{move_tr!("ref-level")}</th>
                                        <th>{move_tr!("prof-bonus")}</th>
                                        <th>{move_tr!("ref-features")}</th>
                                        {has_spells.then(|| view! {
                                            <th>{move_tr!("ref-cantrips")}</th>
                                            <th>{move_tr!("ref-spells-known")}</th>
                                            {(1..=max_spell_level).map(|sl| {
                                                view! { <th>{format!("{sl}")}</th> }
                                            }).collect_view()}
                                        })}
                                        {field_columns.iter().map(|fc| {
                                            let label = fc.label.clone();
                                            view! { <th>{label}</th> }
                                        }).collect_view()}
                                    </tr>
                                </thead>
                                <tbody>
                                    {table_rows.into_iter().map(|(level, prof_bonus, features, cantrips, spells_known, slots, field_values)| {
                                        view! {
                                            <tr>
                                                <td>{level}</td>
                                                <td>{format_bonus(prof_bonus)}</td>
                                                <td class="features-cell">{
                                                    features.into_iter().enumerate().map(|(i, (feat_name, label))| {
                                                        view! {
                                                            {(i > 0).then_some(", ")}
                                                            <a href=hash_href(&format!("feat-{feat_name}")) rel="external">{label}</a>
                                                        }
                                                    }).collect_view()
                                                }</td>
                                                {has_spells.then(|| view! {
                                                    <td>{cantrips.map(|v| v.to_string()).unwrap_or_else(|| "\u{2014}".into())}</td>
                                                    <td>{spells_known.map(|v| v.to_string()).unwrap_or_else(|| "\u{2014}".into())}</td>
                                                    {(1..=max_spell_level).map(|sl| {
                                                        let val = slots.get(sl - 1).copied().unwrap_or(0);
                                                        view! { <td>{if val > 0 { val.to_string() } else { "\u{2014}".into() }}</td> }
                                                    }).collect_view()}
                                                })}
                                                {field_values.into_iter().map(|v| {
                                                    view! { <td>{v}</td> }
                                                }).collect_view()}
                                            </tr>
                                        }
                                    }).collect_view()}
                                </tbody>
                            </table>
                        </div>

                        <h2>{move_tr!("ref-features")}</h2>
                        <ReferenceFeaturesView features=class_features anchors=true />

                        <ReferenceFeaturesView features=subclass_features anchors=true />

                        {if subclass_list.is_empty() {
                            None
                        } else {
                            let cards = subclass_list.into_iter().map(|(sc_name, sc_label, sc_desc)| {
                                let href = format!("{BASE_URL}/r/class/{name_for_link}/{sc_name}");
                                view! {
                                    <A href=href attr:class="reference-card">
                                        <h3>{sc_label}</h3>
                                        <p>{sc_desc}</p>
                                    </A>
                                }
                            }).collect_view();
                            Some(view! {
                                <h2>{move_tr!("ref-subclasses")}</h2>
                                <div class="reference-card-grid">
                                    {cards}
                                </div>
                            })
                        }}
                    </div>
                }
                .into_any()
            })
            .unwrap_or_else(|| {
                view! { <p class="reference-loading">{move_tr!("ref-loading")}</p> }.into_any()
            })
    };

    view! {
        <Title text=Signal::derive(move || i18n.tr("ref-classes")) />
        <div class="reference-layout">
            <ReferenceSidebar current_label>
                {move || registry.with_class_entries(|entries| {
                    entries.values().map(|entry| {
                        let name = entry.name.clone();
                        let label = entry.label().to_string();
                        view! {
                            <A href=format!("{BASE_URL}/r/class/{name}") attr:class="reference-nav-item">
                                {label}
                            </A>
                        }
                    }).collect_view()
                })}
            </ReferenceSidebar>
            <main class="reference-main">
                {detail}
            </main>
        </div>
    }
}
