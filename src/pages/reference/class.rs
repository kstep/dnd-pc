use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::{hooks::use_params, params::Params};

use crate::{
    components::{markdown::Markdown, ref_link::Ref, spinner::Spinner},
    expr::Context as _,
    hooks::use_hash_href,
    model::{Attribute, format_bonus, proficiency_bonus_for_level},
    pages::reference::{
        RefSidebarEntries, ReferenceFeaturesView, ReferenceSidebar, collect_feature_views,
        encode_name,
        progression::{preview_pool_values, preview_progression},
    },
    rules::{DefinitionStore, IndexEntry, PoolSummarizer, RulesRegistry, eval_at_levels},
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

    let current_label = Signal::derive(move || {
        registry
            .entry_label_desc(IndexEntry::Class(&class_name()))
            .0
            .get()
    });
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
        // (nested LocalResource reads inside with don't reliably track)
        registry.with_features_index(|_| {});

        let prerequisites = registry.with_class_defs(|defs| {
            defs.get(name.as_str())
                .and_then(|def| def.prerequisites.as_ref().map(|expr| expr.to_string()))
                .unwrap_or_default()
        });

        registry
            .classes().lookup(&name, |loc| {
                let def = loc.data;
                let subclass_def =
                    subname.as_deref().and_then(|sn| def.subclasses.get(sn));

                let class_label = loc.label().to_string();
                let subclass_label = subname
                    .as_deref()
                    .and_then(|sn| loc.subclass(sn))
                    .map(|sub| sub.label().to_string());
                let title = match &subclass_label {
                    Some(sub_label) => format!("{class_label} \u{2014} {sub_label}"),
                    None => class_label.clone(),
                };
                let description = loc.description().to_string();

                // Hit-die size lives on the `Class Proficiencies (X)` feature
                // as a `HIT_DICE.SIDES = N` assign — eval the OnFeatureAdd
                // pass against an empty context to read it back.
                // Single pass over the class's features:
                // - first feature with `spells` → spell progression preview;
                // - first feature whose OnFeatureAdd writes HIT_DICE.SIDES → die size.
                let (spell_list_name, spell_progression, hit_die) =
                    registry.with_features_index(|features_index| {
                        let mut spell_progression = None;
                        let mut spell_list = None;
                        let mut hit_sides = 0_i32;
                        for feat_name in def.feature_names(subname.as_deref()) {
                            let Some(feat) = features_index.get(feat_name) else {
                                continue;
                            };
                            if let Some(spells) = feat.spells.as_ref() {
                                if spell_progression.is_none() {
                                    spell_progression = Some(preview_progression(feat));
                                }
                                if spell_list.is_none() {
                                    spell_list = spells
                                        .list
                                        .ref_name()
                                        .map(|short: &str| short.to_string());
                                }
                            }
                            if hit_sides == 0 {
                                let ctx = eval_at_levels(feat, |_, _| {});
                                if let Ok(sides) = ctx.resolve(Attribute::HitDiceSides) {
                                    hit_sides = sides;
                                }
                            }
                        }
                        let hit_die = (hit_sides > 0).then(|| format!("d{hit_sides}"));
                        (spell_list, spell_progression, hit_die)
                    });

                // Per-column source: assign-derived pools (Points/Die/Bonus/
                // Choice). All Choice scaling now flows through CHOICE.<n>.COUNT
                // assigns and shows up here uniformly via `PoolKind::Choice`.
                struct FieldColumn {
                    label: String,
                    per_level: Vec<String>,
                }
                let field_columns: Vec<FieldColumn> = registry.with_features_index(|features_index| {
                    let mut columns: Vec<FieldColumn> = Vec::new();
                    for feat_name in def.feature_names(subname.as_deref()) {
                        let Some(feat_def) = features_index.get(feat_name) else { continue };
                        let pools = PoolSummarizer::new(feat_def).pools();
                        if pools.is_empty() {
                            continue;
                        }
                        let feature_level = def.feature_level(subname.as_deref(), feat_name);
                        let per_level_per_pool =
                            preview_pool_values(feat_def, &pools, feature_level);
                        for (pool_idx, pool) in pools.iter().enumerate() {
                            let label = registry
                                .features()
                                .lookup_untracked(feat_name, |loc| {
                                    loc.action(pool.name)
                                        .map(|action| action.label().to_string())
                                })
                                .flatten()
                                .unwrap_or_else(|| pool.name.to_string());
                            let per_level: Vec<String> = per_level_per_pool
                                .iter()
                                .map(|row| row[pool_idx].clone())
                                .collect();
                            // Skip pools that never produce a value.
                            if per_level.iter().all(|cell| cell == "\u{2014}") {
                                continue;
                            }
                            columns.push(FieldColumn { label, per_level });
                        }
                    }
                    columns
                });

                // Build progression table rows
                let table_rows: Vec<_> = (1..=20u32)
                    .map(|level| {
                        let prof_bonus = proficiency_bonus_for_level(level);

                        let mut features: Vec<(String, String)> = Vec::new();
                        if let Some(rules) = def.levels.get(&level) {
                            for feat_name in &rules.features {
                                let label = registry
                                    .features()
                                    .lookup_untracked(feat_name.as_str(), |loc| {
                                        loc.label().to_string()
                                    })
                                    .unwrap_or_else(|| feat_name.clone());
                                features.push((feat_name.clone(), label));
                            }
                        }
                        if let Some(sc) = subclass_def
                            && let Some(sc_rules) = sc.levels.get(&level)
                        {
                            for feat_name in &sc_rules.features {
                                let label = registry
                                    .features()
                                    .lookup_untracked(feat_name.as_str(), |loc| {
                                        loc.label().to_string()
                                    })
                                    .unwrap_or_else(|| feat_name.clone());
                                features.push((feat_name.clone(), label));
                            }
                        }

                        let field_values: Vec<String> = field_columns
                            .iter()
                            .map(|column| column.per_level[(level - 1) as usize].clone())
                            .collect();

                        let progression_row = spell_progression
                            .as_ref()
                            .map(|preview| preview.rows[(level - 1) as usize].clone());

                        (level, prof_bonus, features, field_values, progression_row)
                    })
                    .collect();
                let progression_meta = spell_progression.as_ref().map(|preview| {
                    (
                        preview.max_slot_level,
                        preview.has_cantrips,
                        preview.has_ready,
                        preview.has_known,
                    )
                });

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
                        .map(|sub_def| {
                            let sub_loc = loc
                                .subclass(&sub_def.name)
                                .expect("subclass lookup matches");
                            (
                                sub_def.name.to_string(),
                                sub_loc.label().to_string(),
                                sub_loc.description().to_string(),
                            )
                        })
                        .collect()
                } else {
                    Vec::new()
                };

                let subclass_desc = subname
                    .as_deref()
                    .and_then(|sn| loc.subclass(sn))
                    .map(|sub| sub.description().to_string())
                    .unwrap_or_default();

                let name_for_link = name.clone();

                view! {
                    <Title text=title.clone() />
                    <div class="reference-detail">
                        <h1>{if let Some(sc_label) = subclass_label {
                            let class_href = format!("/r/class/{name}");
                            Either::Left(view! {
                                <Ref href=class_href>{class_label}</Ref>
                                {" \u{2014} "}
                                {sc_label}
                            })
                        } else {
                            Either::Right(class_label)
                        }}</h1>
                        <Markdown text=description />

                        {(!subclass_desc.is_empty()).then(|| view! {
                            <Markdown text=subclass_desc />
                        })}

                        <div class="reference-info-bar">
                            {hit_die.map(|hd| view! {
                                <div class="info-item">
                                    <span class="info-label">{move_tr!("ref-hit-die")}</span>
                                    <span class="info-value">{hd}</span>
                                </div>
                            })}
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
                                        <Ref href=format!("/r/spell/{sln}")>
                                            {move_tr!("ref-spell-list-link")}
                                        </Ref>
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
                                        {progression_meta.map(|(max_slot_level, has_cantrips, has_ready, has_known)| view! {
                                            {has_cantrips.then(|| view! { <th>{move_tr!("ref-cantrips")}</th> })}
                                            {has_ready.then(|| view! { <th>{move_tr!("ref-spells-ready")}</th> })}
                                            {has_known.then(|| view! { <th>{move_tr!("ref-spells-known")}</th> })}
                                            {(1..=max_slot_level).map(|slot_level| {
                                                view! { <th>{slot_level}</th> }
                                            }).collect_view()}
                                        })}
                                        {field_columns.iter().map(|fc| {
                                            let label = fc.label.clone();
                                            view! { <th>{label}</th> }
                                        }).collect_view()}
                                    </tr>
                                </thead>
                                <tbody>
                                    {table_rows.into_iter().map(|(level, prof_bonus, features, field_values, progression_row)| {
                                        view! {
                                            <tr>
                                                <td>{level}</td>
                                                <td>{format_bonus(prof_bonus)}</td>
                                                <td class="features-cell">{
                                                    features.into_iter().enumerate().map(|(index, (feat_name, label))| {
                                                        view! {
                                                            {(index > 0).then_some(", ")}
                                                            <a href=hash_href(&format!("feat-{feat_name}")) rel="external">{label}</a>
                                                        }
                                                    }).collect_view()
                                                }</td>
                                                {progression_row.zip(progression_meta).map(|(row, (max_slot_level, has_cantrips, has_ready, has_known))| view! {
                                                    {has_cantrips.then(|| view! {
                                                        <td>{if row.cantrips > 0 { Either::Left(row.cantrips) } else { Either::Right("\u{2014}") }}</td>
                                                    })}
                                                    {has_ready.then(|| view! {
                                                        <td>{if row.ready > 0 { Either::Left(row.ready) } else { Either::Right("\u{2014}") }}</td>
                                                    })}
                                                    {has_known.then(|| view! {
                                                        <td>{if row.known > 0 { Either::Left(row.known) } else { Either::Right("\u{2014}") }}</td>
                                                    })}
                                                    {(1..=max_slot_level).map(|slot_level| {
                                                        let total = row.slots[(slot_level - 1) as usize];
                                                        view! { <td>{if total > 0 { Either::Left(total) } else { Either::Right("\u{2014}") }}</td> }
                                                    }).collect_view()}
                                                })}
                                                {field_values.into_iter().map(|value| {
                                                    view! { <td>{value}</td> }
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
                                let href = format!("/r/class/{name_for_link}/{}", encode_name(&sc_name));
                                view! {
                                    <Ref href=href attr:class="reference-card">
                                        <h3>{sc_label}</h3>
                                        <p>{sc_desc}</p>
                                    </Ref>
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
            .unwrap_or_else(|| ().into_any())
    };

    // Defs are eager: spin only while the index itself loads; a name absent
    // from the active package set renders as empty, not an infinite spinner.
    let loading = Signal::derive(move || {
        let name = class_name();
        !name.is_empty() && registry.classes().index().is_pending()
    });

    view! {
        <Spinner loading />
        <Title text=move_tr!(i18n, "ref-classes") />
        <div class="reference-page">
            <div class="reference-layout">
                <ReferenceSidebar current_label>
                    <RefSidebarEntries
                        names=Signal::derive(move || registry.with_class_defs(|defs| {
                            defs.keys().map(|name| name.to_string()).collect()
                        }))
                        kind=|n| IndexEntry::Class(n)
                    />
                </ReferenceSidebar>
                <main class="reference-main">
                    {detail}
                </main>
            </div>
        </div>
    }
}
