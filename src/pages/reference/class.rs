use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::{components::A, hooks::use_params, params::Params};

use super::{
    FeatureChoicesView, FeatureSpells, FeatureSpellsView, ReferenceSidebar, feature_choices,
};
use crate::{
    BASE_URL,
    model::{Translatable, format_bonus},
    rules::{FieldKind, RulesRegistry, SpellList, get_for_level},
};

#[derive(Params, Clone, Debug, PartialEq)]
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
            registry.fetch_class_tracked(&name);
        }
    });

    let current_label = Signal::derive(move || registry.class_label_by_name(&class_name()));

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

        let prerequisites = registry.with_class_entries(|entries| {
            entries
                .iter()
                .find(|e| e.name == name)
                .map(|e| {
                    e.prerequisites
                        .iter()
                        .map(|a| i18n.tr(a.tr_key()))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default()
        });

        registry
            .with_class_tracked(&name, |def| {
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

                let saving_throws = def
                    .saving_throws
                    .iter()
                    .map(|a| i18n.tr(a.tr_key()))
                    .collect::<Vec<_>>()
                    .join(", ");

                let proficiencies = def
                    .proficiencies
                    .iter()
                    .map(|p| i18n.tr(p.tr_key()))
                    .collect::<Vec<_>>()
                    .join(", ");

                // Find spellcasting feature
                let spell_feat =
                    def.features(subname.as_deref()).find(|f| f.spells.is_some());
                let spells_def = spell_feat.and_then(|f| f.spells.as_ref());
                let has_spells = spells_def.is_some();
                let spell_list_name = spells_def.and_then(|sd| match &sd.list {
                    SpellList::Ref { from } => from
                        .strip_prefix("spells/")
                        .and_then(|s| s.strip_suffix(".json"))
                        .map(|s| s.to_string()),
                    _ => None,
                });
                let max_spell_level = spells_def
                    .map(|sd| {
                        sd.levels
                            .iter()
                            .filter_map(|l| l.slots.as_ref())
                            .map(|s| s.len())
                            .max()
                            .unwrap_or(0)
                    })
                    .unwrap_or(0);

                // Field columns
                struct FieldColumn<'a> {
                    label: String,
                    kind: &'a FieldKind,
                }
                let field_columns: Vec<FieldColumn<'_>> = def
                    .features(subname.as_deref())
                    .flat_map(|f| {
                        f.fields.values().map(|fd| FieldColumn {
                            label: fd.label().to_string(),
                            kind: &fd.kind,
                        })
                    })
                    .collect();

                // Build progression table rows
                let table_rows: Vec<_> = (1..=20u32)
                    .map(|level| {
                        let prof_bonus = (level as i32 - 1) / 4 + 2;

                        let mut features: Vec<(String, String)> = Vec::new();
                        if let Some(rules) = def.levels.get(level as usize - 1) {
                            for feat_name in &rules.features {
                                let label = def
                                    .features
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
                                let label = sc
                                    .features
                                    .get(feat_name.as_str())
                                    .map(|f| f.label().to_string())
                                    .unwrap_or_else(|| feat_name.clone());
                                features.push((feat_name.clone(), label));
                            }
                        }

                        let spell_level_rules =
                            spells_def.and_then(|sd| sd.levels.get(level as usize - 1));
                        let cantrips = spell_level_rules.and_then(|r| r.cantrips);
                        let spells_known = spell_level_rules.and_then(|r| r.spells);
                        let slots = spell_level_rules
                            .and_then(|r| r.slots.as_deref())
                            .unwrap_or_default();

                        let field_values: Vec<String> = field_columns
                            .iter()
                            .map(|fc| match fc.kind {
                                FieldKind::Points { levels, .. } => {
                                    let v: u32 = get_for_level(levels, level);
                                    if v > 0 {
                                        v.to_string()
                                    } else {
                                        "\u{2014}".into()
                                    }
                                }
                                FieldKind::Die { levels } => {
                                    let v: String = get_for_level(levels, level);
                                    if v.is_empty() {
                                        "\u{2014}".into()
                                    } else {
                                        v
                                    }
                                }
                                FieldKind::Choice { levels, .. } => {
                                    let v: u32 = get_for_level(levels, level);
                                    if v > 0 {
                                        v.to_string()
                                    } else {
                                        "\u{2014}".into()
                                    }
                                }
                                FieldKind::Bonus { levels } => {
                                    let v: i32 = get_for_level(levels, level);
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

                let class_features: Vec<_> = def
                    .features
                    .values()
                    .map(|feat| {
                        let spells = FeatureSpells::from_spell_list(
                            feat.spells.as_ref().map(|spells_def| &spells_def.list),
                        );
                        let choices = feature_choices(&feat.fields);
                        let langs = feat.languages.join(", ");
                        (
                            feat.name.clone(),
                            feat.label().to_string(),
                            feat.description.clone(),
                            langs,
                            spells,
                            choices,
                        )
                    })
                    .collect();

                let subclass_features: Vec<_> = subclass_def
                    .map(|sc| {
                        sc.features
                            .values()
                            .map(|feat| {
                                let spells = FeatureSpells::from_spell_list(
                                    feat.spells.as_ref().map(|spells_def| &spells_def.list),
                                );
                                let choices = feature_choices(&feat.fields);
                                let langs = feat.languages.join(", ");
                                (
                                    feat.name.clone(),
                                    feat.label().to_string(),
                                    feat.description.clone(),
                                    langs,
                                    spells,
                                    choices,
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();

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
                            view! {
                                <A href=class_href>{class_label}</A>
                                {" \u{2014} "}
                                {sc_label}
                            }.into_any()
                        } else {
                            view! { {class_label} }.into_any()
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
                            <div class="info-item">
                                <span class="info-label">{move_tr!("panel-saving-throws")}</span>
                                <span class="info-value">{saving_throws}</span>
                            </div>
                            <div class="info-item">
                                <span class="info-label">{move_tr!("proficiencies")}</span>
                                <span class="info-value">{proficiencies}</span>
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
                                                            <a href=format!("#feat-{feat_name}")>{label}</a>
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
                        <div class="reference-features">
                            {class_features
                                .into_iter()
                                .map(|(feat_name, label, desc, langs, spells, choices)| {
                                    let anchor_id = format!("feat-{feat_name}");
                                    view! {
                                        <div class="reference-feature" id=anchor_id>
                                            <h3>{label}</h3>
                                            <p>{desc}</p>
                                            {(!langs.is_empty()).then(|| view! {
                                                <p class="feature-languages">
                                                    {move_tr!("ref-languages")}{": "}{langs}
                                                </p>
                                            })}
                                            <FeatureSpellsView spells=spells />
                                            <FeatureChoicesView choices=choices />
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>

                        {(!subclass_features.is_empty()).then(|| view! {
                            <div class="reference-features">
                                {subclass_features
                                    .into_iter()
                                    .map(|(feat_name, label, desc, langs, spells, choices)| {
                                        let anchor_id = format!("feat-{feat_name}");
                                        view! {
                                            <div class="reference-feature" id=anchor_id>
                                                <h3>{label}</h3>
                                                <p>{desc}</p>
                                                {(!langs.is_empty()).then(|| view! {
                                                    <p class="feature-languages">
                                                        {move_tr!("ref-languages")}{": "}{langs}
                                                    </p>
                                                })}
                                                <FeatureSpellsView spells=spells />
                                                <FeatureChoicesView choices=choices />
                                            </div>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        })}

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
        <div class="reference-page">
            <div class="reference-layout">
                <ReferenceSidebar current_label>
                    {move || registry.with_class_entries(|entries| {
                        entries.iter().map(|entry| {
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
        </div>
    }
}
