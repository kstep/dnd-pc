use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::{
        datalist::{DatalistInput, DatalistOption, SharedDatalist, next_datalist_id},
        entry_name::EntryName,
        icon::Icon,
        markdown::Markdown,
        ref_link::Ref,
        slot_box::SlotBox,
        spell_info_bar::SpellInfoBar,
        toggle_button::ToggleButton,
    },
    model::{
        Ability, Character, CharacterStoreFields, FeaturesStoreFields, Spell, SpellData,
        SpellSlotPool, Translatable, format_bonus,
    },
    rules::{RulesRegistry, SpellMeta},
};

fn lookup_spell_meta(registry: RulesRegistry, spell_name: &str) -> Option<SpellMeta> {
    if spell_name.is_empty() {
        return None;
    }
    registry.with_spells_index(|index| index.get(spell_name).map(|def| def.meta()))
}

fn update_spells(
    feat_name: StoredValue<String>,
    store: Store<Character>,
    f: impl FnOnce(&mut SpellData),
) {
    feat_name.with_value(|key| {
        store.features().data().update(|map| {
            if let Some(sc) = map.get_mut(key).and_then(|entry| entry.spells.as_mut()) {
                f(sc);
            }
        });
    });
}

fn update_spell(
    feat_name: StoredValue<String>,
    store: Store<Character>,
    index: usize,
    f: impl FnOnce(&mut Spell),
) {
    update_spells(feat_name, store, |sc| {
        if let Some(spell) = sc.spells.get_mut(index) {
            f(spell);
        }
    });
}

fn read_spell<T: Default>(
    feat_name: StoredValue<String>,
    store: Store<Character>,
    index: usize,
    f: impl FnOnce(&Spell) -> T,
) -> T {
    feat_name.with_value(|key| {
        store
            .features()
            .data()
            .read()
            .get(key)
            .and_then(|entry| entry.spells.as_ref())
            .and_then(|sc| sc.spells.get(index))
            .map(f)
            .unwrap_or_default()
    })
}

fn update_known_spell(
    feat_name: StoredValue<String>,
    store: Store<Character>,
    index: usize,
    f: impl FnOnce(&mut Spell),
) {
    update_spells(feat_name, store, |sc| {
        if let Some(known) = &mut sc.known
            && let Some(spell) = known.get_mut(index)
        {
            f(spell);
        }
    });
}

fn lookup_pick(
    options: Signal<Vec<DatalistOption>>,
    resolved: Option<&str>,
) -> (String, Option<u32>) {
    resolved
        .and_then(|name| {
            options.with(|opts| {
                opts.iter()
                    .find(|opt| opt.name == name)
                    .map(|opt| (opt.description.get_untracked(), opt.count))
            })
        })
        .unwrap_or_default()
}

fn apply_spell_pick(
    spell: &mut Spell,
    input: String,
    resolved: Option<String>,
    desc: String,
    level: Option<u32>,
) {
    if let Some(name) = resolved {
        if let Some(level) = level {
            spell.level = level;
        }
        spell.name = name;
        spell.label = Some(input);
    } else {
        spell.set_label(input);
    }
    spell.description = desc;
}

fn read_known_spell<T: Default>(
    feat_name: StoredValue<String>,
    store: Store<Character>,
    index: usize,
    f: impl FnOnce(&Spell) -> T,
) -> T {
    feat_name.with_value(|key| {
        store
            .features()
            .data()
            .read()
            .get(key)
            .and_then(|entry| entry.spells.as_ref())
            .and_then(|sc| sc.known.as_ref())
            .and_then(|known| known.get(index))
            .map(f)
            .unwrap_or_default()
    })
}

#[component]
fn FeatureSpellcastingSection(
    #[prop(into)] feature_name: String,
    default_ability: Ability,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();
    let i18n = expect_context::<leptos_fluent::I18n>();

    // Resolve feature name → label and cost suffix for display
    let panel_title = registry
        .features()
        .lookup_untracked(&feature_name, |loc| loc.label().to_string())
        .unwrap_or_else(|| feature_name.clone());
    let cost_short: String = registry
        .with_feature(&feature_name, |feat| {
            feat.cost_info().map(|(_, short)| short.to_string())
        })
        .flatten()
        .unwrap_or_default();
    let has_cost_field = !cost_short.is_empty();
    let cost_short = StoredValue::new(cost_short);
    let feat_name = StoredValue::new(feature_name);

    let casting_ability = Memo::new(move |_| {
        feat_name.with_value(|key| {
            store
                .features()
                .data()
                .read()
                .get(key)
                .and_then(|e| e.spells.as_ref())
                .map(|sc| sc.casting_ability)
                .unwrap_or(default_ability)
        })
    });
    let spell_save_dc = Memo::new(move |_| store.read().spell_save_dc(casting_ability.get()));
    let spell_attack = Memo::new(move |_| store.read().spell_attack_bonus(casting_ability.get()));

    let is_two_tier = Memo::new(move |_| {
        feat_name.with_value(|key| {
            store
                .features()
                .data()
                .read()
                .get(key)
                .and_then(|e| e.spells.as_ref())
                .is_some_and(|sc| sc.is_two_tier())
        })
    });

    // Per-level spell suggestions from registry (for spellbook / single-tier)
    let spell_suggestions: [RwSignal<Vec<DatalistOption>>; 10] =
        std::array::from_fn(|_| RwSignal::new(Vec::new()));
    Effect::new(move || {
        registry.track_spell_cache();
        let mut data = feat_name.with_value(|key| resolve_feature_spell_list(&registry, key));
        for (level, signal) in spell_suggestions.iter().enumerate() {
            signal.set(std::mem::take(&mut data[level]));
        }
    });

    let pool = Memo::new(move |_| {
        feat_name.with_value(|key| {
            store
                .features()
                .data()
                .read()
                .get(key)
                .and_then(|e| e.spells.as_ref())
                .map(|sc| sc.pool)
                .unwrap_or(SpellSlotPool::Arcane)
        })
    });

    let max_slot_level = Memo::new(move |_| {
        store
            .spell_slots()
            .read()
            .get(&pool.get())
            .and_then(|slots| {
                slots
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, slot)| slot.total > 0)
                    .map(|(idx, _)| (idx as u32) + 1)
            })
            .unwrap_or(0)
    });

    let pick_suggestions = move |suggestions: &[RwSignal<Vec<DatalistOption>>; 10]| {
        let max = max_slot_level.get() as usize;
        suggestions[1..=max]
            .iter()
            .fold(Vec::new(), |mut acc, sig| {
                sig.with(|v| acc.extend_from_slice(v));
                acc
            })
    };

    let leveled_suggestions = Memo::new(move |_| pick_suggestions(&spell_suggestions));

    // Per-level suggestions from known (spellbook) entries for prepared spells
    let known_suggestions: [RwSignal<Vec<DatalistOption>>; 10] =
        std::array::from_fn(|_| RwSignal::new(Vec::new()));

    let leveled_known = Memo::new(move |_| pick_suggestions(&known_suggestions));

    let pick_options = move |level: u32, prefer_known: bool| -> Signal<Vec<DatalistOption>> {
        if level == 0 {
            spell_suggestions[0].into()
        } else if prefer_known {
            leveled_known.into()
        } else {
            leveled_suggestions.into()
        }
    };

    // Three shared <datalist> elements per section — one per options-bucket
    // (cantrip, full class spell list, spellbook subset). N spell entries
    // referencing the same bucket all read from one native datalist.
    let cantrip_list_id = next_datalist_id();
    let leveled_list_id = next_datalist_id();
    let known_list_id = next_datalist_id();
    let cantrip_options: Signal<Vec<DatalistOption>> = spell_suggestions[0].into();
    let leveled_options: Signal<Vec<DatalistOption>> = leveled_suggestions.into();
    let known_options: Signal<Vec<DatalistOption>> = leveled_known.into();
    // Callback is Copy — both spellbook and prepared iteration closures
    // (separate `move ||` inside the same view!) can share it without clones.
    let pick_list_id: Callback<(u32, bool), String> = Callback::new({
        let cantrip_id = cantrip_list_id.clone();
        let leveled_id = leveled_list_id.clone();
        let known_id = known_list_id.clone();
        move |(level, prefer_known)| {
            if level == 0 {
                cantrip_id.clone()
            } else if prefer_known {
                known_id.clone()
            } else {
                leveled_id.clone()
            }
        }
    });
    Effect::new(move || {
        let guard = store.features().data().read();
        let known = feat_name.with_value(|key| {
            guard
                .get(key)
                .and_then(|e| e.spells.as_ref())
                .and_then(|sc| sc.known.as_ref())
        });
        let mut by_level: [Vec<DatalistOption>; 10] = Default::default();
        if let Some(known) = known {
            for spell in known.iter().filter(|s| !s.name.is_empty()) {
                let level = spell.level.min(9);
                if let Some(bucket) = by_level.get_mut(level as usize) {
                    bucket.push(
                        DatalistOption::new(&spell.name, spell.label(), &spell.description)
                            .with_count(level),
                    );
                }
            }
        }
        for (level, signal) in known_suggestions.iter().enumerate() {
            signal.set(std::mem::take(&mut by_level[level]));
        }
    });

    let anchor_id = feat_name.with_value(|n| n.clone());
    let char_id = store.read_untracked().id;
    let build_anchor = feat_name.with_value(|n| {
        store
            .read_untracked()
            .features
            .iter()
            .find(|f| f.name.as_str() == n.as_str())
            .map(|f| f.dom_id())
            .unwrap_or_else(|| n.clone())
    });
    let build_href = format!("/c/{char_id}/build#{build_anchor}");

    view! {
        <section id=anchor_id class="spellcasting-section">
            <SharedDatalist id=cantrip_list_id.clone() options=cantrip_options />
            <SharedDatalist id=leveled_list_id.clone() options=leveled_options />
            <SharedDatalist id=known_list_id.clone() options=known_options />
            <div class="section-header">
                <h3>{panel_title}</h3>
                <Ref href=build_href scroll=false attr:class="entry-spell-link">
                    "← "{move_tr!("tab-build")}
                </Ref>
            </div>

            <div class="slot-box-list">
                <SlotBox label=move_tr!("casting-ability")>
                    <select
                        on:change=move |e| {
                            let value = event_target_value(&e);
                            if let Some(ability) = Ability::from_u8_str(&value) {
                                update_spells(feat_name, store, |sc| sc.casting_ability = ability);
                            }
                        }
                    >
                        {Ability::iter()
                            .map(|ability| {
                                let tr_key = ability.tr_key();
                                let option_value = (ability as u8).to_string();
                                let selected = ability == default_ability;
                                let label = Signal::derive(move || i18n.tr(tr_key));
                                view! {
                                    <option value=option_value selected=selected>
                                        {label}
                                    </option>
                                }
                            })
                            .collect_view()}
                    </select>
                </SlotBox>
                <SlotBox label=move_tr!("spell-save-dc")>
                    <span class="stat-highlight">
                        {move || spell_save_dc.get().to_string()}
                    </span>
                </SlotBox>
                <SlotBox label=move_tr!("spell-attack")>
                    <span class="stat-highlight">
                        {move || format_bonus(spell_attack.get())}
                    </span>
                </SlotBox>
            </div>

            // Spellbook section (only for two-tier casters like Wizard)
            <Show when=move || is_two_tier.get()>
                <div class="section-header">
                    <h4>{move_tr!("spellbook")}</h4>
                    <button
                        class="btn-toggle-desc"
                        on:click=move |_| {
                            update_spells(feat_name, store, |sc| {
                                if let Some(known) = &mut sc.known {
                                    known.sort_by(|a, b| {
                                        b.sticky
                                            .cmp(&a.sticky)
                                            .then_with(|| a.level.cmp(&b.level))
                                            .then_with(|| a.name.cmp(&b.name))
                                    });
                                }
                            });
                        }
                    >
                        <Icon name="arrow-down-a-z" size=16 />
                    </button>
                </div>
                <div class="entry-list">
                    {move || {
                        let guard = store.features().data().read();
                        feat_name.with_value(|key| {
                            guard
                                .get(key)
                                .and_then(|e| e.spells.as_ref())
                                .and_then(|sc| sc.known.as_ref())
                        }).map(|known| known
                            .iter()
                            .enumerate()
                            .map(|(i, spell)| {
                                let spell_name = spell.name.clone();
                                let spell_label = spell.label().to_string();
                                let spell_level = spell.level.to_string();
                                let spell_sticky = spell.sticky;
                                let options = pick_options(spell.level, false);
                                // Spellbook spells autocomplete from the full class
                                // spell list (prefer_known = false).
                                let list_id = pick_list_id.run((spell.level, false));
                                view! {
                                    <div class="entry-item">
                                        <ToggleButton />
                                        <div class="entry-content">
                                            {if spell_sticky {
                                                Either::Left(view! {
                                                    <EntryName>{spell_label.clone()}</EntryName>
                                                })
                                            } else {
                                                Either::Right(view! {
                                                    <DatalistInput
                                                        value=spell_label
                                                        placeholder=move_tr!("spell-name")
                                                        class="entry-name"
                                                        list_id=list_id
                                                        options=options
                                                        badge_key="spell-level-badge"
                                                        on_input=move |input, resolved| {
                                                            let (desc, level) = lookup_pick(options, resolved.as_deref());
                                                            update_known_spell(feat_name, store, i, |spell| {
                                                                apply_spell_pick(spell, input, resolved, desc, level);
                                                            });
                                                        }
                                            />
                                                })
                                            }}
                                            <input
                                                type="number"
                                                class="short-input"
                                                min="0"
                                                max="9"
                                                placeholder="Lv"
                                                disabled=spell_sticky
                                                prop:value=spell_level
                                                on:change=move |e| {
                                                    if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                        update_known_spell(feat_name, store, i, |spell| spell.level = value);
                                                    }
                                                }
                                            />
                                        </div>
                                        <div class="entry-actions">
                                            <Show when=move || !spell_sticky>
                                                <button
                                                    class="btn-remove"
                                                    on:click=move |_| {
                                                        update_spells(feat_name, store, |sc| {
                                                            if let Some(known) = &mut sc.known
                                                                && i < known.len()
                                                            {
                                                                known.remove(i);
                                                            }
                                                        });
                                                    }
                                                >
                                                    <Icon name="x" />
                                                </button>
                                            </Show>
                                        </div>
                                        {
                                            let meta = lookup_spell_meta(registry, &spell_name);
                                            view! {
                                                {meta.map(|meta| view! { <SpellInfoBar meta /> })}
                                                {if meta.is_some() {
                                                    Either::Left(view! {
                                                        <div class="entry-desc">
                                                            <Markdown text=move || read_known_spell(feat_name, store, i, |spell| spell.description.clone()) />
                                                        </div>
                                                    })
                                                } else {
                                                    Either::Right(view! {
                                                        <textarea
                                                            class="entry-desc"
                                                            placeholder=move_tr!("description")
                                                            prop:value=move || read_known_spell(feat_name, store, i, |spell| spell.description.clone())
                                                            on:change=move |e| {
                                                                let value = event_target_value(&e);
                                                                update_known_spell(feat_name, store, i, |spell| spell.description = value);
                                                            }
                                                        />
                                                    })
                                                }}
                                            }
                                        }
                                    </div>
                                }
                            })
                            .collect_view())
                    }}
                </div>
                <button
                    class="btn-primary"
                    on:click=move |_| {
                        update_spells(feat_name, store, |sc| {
                            if let Some(known) = &mut sc.known {
                                known.push(Spell::default());
                            }
                        });
                    }
                >
                    {move_tr!("btn-add-spell")}
                </button>
            </Show>

            // Prepared spells section (or single-tier spell list)
            <div class="section-header">
                <h4>{move || if is_two_tier.get() { move_tr!("prepared-spells") } else { move_tr!("spells") }}</h4>
                <button
                    class="btn-toggle-desc"
                    on:click=move |_| {
                        update_spells(feat_name, store, |sc| {
                            sc.spells.sort_by(|a, b| {
                                b.sticky
                                    .cmp(&a.sticky)
                                    .then_with(|| a.level.cmp(&b.level))
                                    .then_with(|| {
                                        a.name.cmp(&b.name)
                                    })
                            });
                        });
                    }
                >
                    <Icon name="arrow-down-a-z" size=16 />
                </button>
            </div>
            <div class="entry-list">
                {move || {
                    let guard = store.features().data().read();
                    let two_tier = is_two_tier.get();
                    feat_name.with_value(|key| {
                        guard
                            .get(key)
                            .and_then(|e| e.spells.as_ref())
                    }).map(|sc| sc.spells
                        .iter()
                        .enumerate()
                        .map(|(i, spell)| {
                            let spell_name = spell.name.clone();
                            let spell_label = spell.label().to_string();
                            let spell_level = spell.level.to_string();
                            let spell_sticky = spell.sticky;
                            let has_free_uses = spell.free_uses.is_some();
                            // Two-tier: autocomplete from spellbook; single-tier/cantrips: from registry
                            let options = pick_options(spell.level, two_tier);
                            // Prepared spells: two-tier casters autocomplete from
                            // the spellbook subset, single-tier from the full list.
                            let list_id = pick_list_id.run((spell.level, two_tier));
                            view! {
                                <div class="entry-item">
                                    <ToggleButton />
                                    <div class="entry-content">
                                        {if spell_sticky {
                                            Either::Left(view! {
                                                <EntryName>{spell_label.clone()}</EntryName>
                                            })
                                        } else {
                                            Either::Right(view! {
                                                <DatalistInput
                                                    value=spell_label
                                                    placeholder=move_tr!("spell-name")
                                                    class="entry-name"
                                                    list_id=list_id
                                                    options=options
                                                    badge_key="spell-level-badge"
                                                    on_input=move |input, resolved| {
                                                        let (desc, level) = lookup_pick(options, resolved.as_deref());
                                                        update_spell(feat_name, store, i, |spell| {
                                                            apply_spell_pick(spell, input, resolved, desc, level);
                                                        });
                                                    }
                                                />
                                            })
                                        }}
                                        <input
                                            type="number"
                                            class="short-input"
                                            min="0"
                                            max="9"
                                            placeholder="Lv"
                                            disabled=spell_sticky
                                            prop:value=spell_level
                                            on:change=move |e| {
                                                if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                    update_spell(feat_name, store, i, |spell| spell.level = value);
                                                }
                                            }
                                        />
                                    </div>
                                    <div class="entry-actions">
                                        <Show when=move || !spell_sticky>
                                            <button
                                                class="btn-remove"
                                                on:click=move |_| {
                                                    update_spells(feat_name, store, |sc| {
                                                        if i < sc.spells.len() {
                                                            sc.spells.remove(i);
                                                        }
                                                    });
                                                }
                                            >
                                                <Icon name="x" />
                                            </button>
                                        </Show>
                                    </div>
                                    <Show when=move || has_free_uses || has_cost_field>
                                        <div class="entry-full-row spell-cost-row">
                                            <Show when=move || has_free_uses>
                                                <SlotBox label=move_tr!("free-uses")>
                                                    <input
                                                        type="number"
                                                        min="0"
                                                        prop:value=move || read_spell(feat_name, store, i, |spell| {
                                                            spell.free_uses.as_ref().map(|fu| fu.used.to_string()).unwrap_or_default()
                                                        })
                                                        on:change=move |e| {
                                                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                                update_spell(feat_name, store, i, |spell| {
                                                                    if let Some(fu) = &mut spell.free_uses {
                                                                        fu.used = value;
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    />
                                                    " / "
                                                    <input
                                                        type="number"
                                                        min="0"
                                                        prop:value=move || read_spell(feat_name, store, i, |spell| {
                                                            spell.free_uses.as_ref().map(|fu| fu.max.to_string()).unwrap_or_default()
                                                        })
                                                        on:change=move |e| {
                                                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                                update_spell(feat_name, store, i, |spell| {
                                                                    if let Some(fu) = &mut spell.free_uses {
                                                                        fu.max = value;
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    />
                                                </SlotBox>
                                            </Show>
                                            <SlotBox label=move_tr!("cost")>
                                                <input
                                                    type="number"
                                                    min="0"
                                                    prop:value=move || read_spell(feat_name, store, i, |spell| spell.cost.to_string())
                                                    on:change=move |e| {
                                                        if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                            update_spell(feat_name, store, i, |spell| spell.cost = value);
                                                        }
                                                    }
                                                />
                                                <Show when=move || has_cost_field>
                                                    {cost_short.get_value()}
                                                </Show>
                                            </SlotBox>
                                        </div>
                                    </Show>
                                        {
                                            let meta = lookup_spell_meta(registry, &spell_name);
                                            view! {
                                                {meta.map(|meta| view! { <SpellInfoBar meta /> })}
                                                {if meta.is_some() {
                                                    Either::Left(view! {
                                                        <div class="entry-desc">
                                                            <Markdown text=move || read_spell(feat_name, store, i, |spell| spell.description.clone()) />
                                                        </div>
                                                    })
                                                } else {
                                                    Either::Right(view! {
                                                        <textarea
                                                            class="entry-desc"
                                                            placeholder=move_tr!("description")
                                                            prop:value=move || read_spell(feat_name, store, i, |spell| spell.description.clone())
                                                            on:change=move |e| {
                                                                let value = event_target_value(&e);
                                                                update_spell(feat_name, store, i, |spell| spell.description = value);
                                                            }
                                                        />
                                                    })
                                                }}
                                            }
                                        }
                                </div>
                            }
                        })
                        .collect_view())
                }}
            </div>
            <button
                class="btn-primary"
                on:click=move |_| {
                    update_spells(feat_name, store, |sc| sc.spells.push(Spell::default()));
                }
            >
                {move_tr!("btn-add-spell")}
            </button>
        </section>
    }
}

/// Resolve the spell list for a given feature into per-level buckets.
fn resolve_feature_spell_list(
    registry: &RulesRegistry,
    feature_name: &str,
) -> [Vec<DatalistOption>; 10] {
    registry
        .with_feature(feature_name, |feat| {
            let spells_def = feat.spells.as_ref()?;
            let mut by_level: [Vec<DatalistOption>; 10] = Default::default();
            registry.with_spell_list_untracked(&spells_def.list, |iter| {
                for spell in iter {
                    if let Some(bucket) = by_level.get_mut(spell.level as usize) {
                        bucket.push(
                            DatalistOption::new(&*spell.name, spell.label(), spell.description())
                                .with_count(spell.level),
                        );
                    }
                }
            });
            Some(by_level)
        })
        .flatten()
        .unwrap_or_default()
}

#[component]
pub fn SpellcastingPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let has_spells = Memo::new(move |_| {
        store
            .features()
            .data()
            .read()
            .values()
            .any(|e| e.spells.is_some())
    });
    let slots_expanded = RwSignal::new(false);

    crate::hooks::use_scroll_to_hash();

    view! {
        <Show when=move || has_spells.get()>
            <section>
                <div class="section-header">
                    <button
                        class="btn-toggle-desc"
                        class:expanded=move || slots_expanded.get()
                        on:click=move |_| slots_expanded.update(|expanded| *expanded = !*expanded)
                    />
                    <h3
                        class="clickable"
                        on:click=move |_| slots_expanded.update(|expanded| *expanded = !*expanded)
                    >
                        {move_tr!("spell-slots")}
                    </h3>
                </div>
                {move || {
                    let expanded = slots_expanded.get();
                    let character = store.read();
                    let pools: Vec<SpellSlotPool> = character.spell_slots.active_pools().collect();
                    let multiple_pools = pools.len() > 1;
                    let i18n = expect_context::<leptos_fluent::I18n>();
                    pools
                        .into_iter()
                        .map(|pool| {
                            let slots: Vec<_> = character.spell_slots.iter_pool(pool).collect();
                            let pool_header = if multiple_pools {
                                Some(view! {
                                    <h5 class="pool-header">{i18n.tr(pool.tr_key())}</h5>
                                })
                            } else {
                                None
                            };
                            view! {
                                {pool_header}
                                <div class="slot-box-list">
                                    {slots
                                        .into_iter()
                                        .filter(|(_, slot)| expanded || slot.total > 0)
                                        .map(|(level, slot)| {
                                            let idx = (level - 1) as usize;
                                            let label = format!("Lv {level}");
                                            view! {
                                                <SlotBox label=label>
                                                    <input
                                                        type="number"
                                                        min="0"
                                                        placeholder=move_tr!("used")
                                                        prop:value=slot.used.to_string()
                                                        on:change=move |e| {
                                                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                                store.spell_slots().update(|pools| {
                                                                    if let Some(slots) = pools.get_mut(&pool) {
                                                                        slots[idx].used = value;
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    />
                                                    " / "
                                                    <input
                                                        type="number"
                                                        min="0"
                                                        placeholder=move_tr!("total")
                                                        prop:value=slot.total.to_string()
                                                        on:change=move |e| {
                                                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                                store.spell_slots().update(|pools| {
                                                                    if let Some(slots) = pools.get_mut(&pool) {
                                                                        slots[idx].total = value;
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    />
                                                </SlotBox>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </section>
            {move || {
                store
                    .features().data()
                    .read()
                    .iter()
                    .filter_map(|(name, entry)| {
                        entry
                            .spells
                            .as_ref()
                            .map(|sc| (name.clone(), sc.casting_ability))
                    })
                    .map(|(feature_name, default_ability)| {
                        view! {
                            <FeatureSpellcastingSection feature_name=feature_name default_ability=default_ability />
                        }
                    })
                    .collect_view()
            }}
        </Show>
    }
}
