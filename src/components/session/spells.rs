use std::collections::BTreeMap;

use leptos::prelude::*;
use leptos_fluent::{move_tr, tr};
use reactive_stores::Store;

use crate::{
    components::{
        cast_button::{CastButton, CastOption},
        effects_calc_modal::{
            EffectsCalcInfo, EffectsCalcModal, all_self_effects_diceless, apply_self_effects_now,
            inject_resource_vars, open_calc_modal,
        },
        icon::Icon,
        session::{FreeUsesBadge, adv_icon},
        session_list::{SessionList, SessionListItem},
    },
    effective::EffectiveCharacter,
    model::{
        Ability, ActionType, Attribute, Character, CharacterCoreStoreFields, CharacterStoreFields,
        CombatStatsStoreFields, EffectDuration, EffectRange, FeatureValue, FeaturesStoreFields,
        SpellSlotPool, format_bonus,
    },
    rules::{CastTime, RulesRegistry},
};

fn format_rounds(rounds: u32) -> String {
    if rounds >= 600 {
        format!("{}h", rounds / 600)
    } else if rounds >= 10 {
        format!("{}m", rounds / 10)
    } else {
        format!("{}r", rounds)
    }
}

#[component]
pub fn SpellsBlock() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let store = expect_context::<Store<Character>>();
    let eff = expect_context::<EffectiveCharacter>();
    let spell_slots = store.core().spell_slots();
    let feature_data = store.core().features().data();

    // Modal state
    let show_calc = RwSignal::new(false);
    let calc_info = StoredValue::new(None::<EffectsCalcInfo>);

    let open_calc = move |spell_name: &str,
                          spell_level: u32,
                          fname: &str,
                          pool: SpellSlotPool,
                          casting_ability: Ability,
                          opt: &CastOption| {
        let slot_level = match opt {
            CastOption::SpellSlot { level, .. } | CastOption::Ritual { level, .. } => *level,
            _ => spell_level,
        };

        let effects = registry.with_spells_index(|index| {
            index
                .get(spell_name)
                .map(|def| def.effects.clone())
                .unwrap_or_default()
        });

        if !effects.is_empty() {
            let character = store.read_untracked();
            let caster_level = character.caster_level(pool);

            let mut extra_vars = BTreeMap::new();
            extra_vars.insert(Attribute::SlotLevel, slot_level as i32);
            extra_vars.insert(Attribute::CasterLevel(None), caster_level as i32);
            extra_vars.insert(
                Attribute::CasterModifier,
                character.ability_modifier(casting_ability),
            );

            // Inject resource field values and spell cost
            if let Some(entry) = character.features.get(fname) {
                inject_resource_vars(&mut extra_vars, entry);
            }
            if let CastOption::PointsCost { cost, .. } = opt {
                extra_vars.insert(Attribute::Cost, *cost as i32);
            }

            // All effects are Caster with no dice — apply immediately, skip modal
            let all_caster = effects.iter().all(|e| e.range.can_target_self());
            if all_caster && all_self_effects_diceless(&effects, &character, &extra_vars) {
                drop(character);
                apply_self_effects_now(
                    &effects,
                    spell_name,
                    fname,
                    &extra_vars,
                    &store,
                    eff.effects(),
                );
                return;
            }

            let spell_label = character
                .features
                .spell_data(fname)
                .and_then(|sd| sd.spells.iter().find(|s| s.name == spell_name))
                .map(|s| s.label().to_string())
                .unwrap_or_else(|| spell_name.to_string());

            let title = if slot_level > 0 {
                format!(
                    "{} ({})",
                    spell_label,
                    tr!("slot-level", {"level" => slot_level})
                )
            } else {
                spell_label
            };

            open_calc_modal(
                show_calc,
                calc_info,
                EffectsCalcInfo {
                    title,
                    effects,
                    extra_vars,
                    spell_name: spell_name.to_string(),
                    feature_name: fname.to_string(),
                },
            );
        }
    };

    let spells_view = move || {
        feature_data
            .read()
            .iter()
            .filter_map(|(name, entry)| {
                let spell_data = entry.spells.as_ref()?;

                let (feature_label, cost_field_name, cost_short) = registry
                    .features()
                    .lookup_untracked(name, |loc| {
                        let label = loc.label().to_string();
                        let (cost_name, cost_short) = loc
                            .data
                            .cost_info()
                            .map(|(name, short)| (name.to_string(), short))
                            .unwrap_or_default();
                        (label, cost_name, cost_short)
                    })
                    .unwrap_or_else(|| (name.to_string(), String::new(), String::new()));
                let has_cost_field = !cost_short.is_empty();
                let cost_field_name = StoredValue::new(cost_field_name);

                let spell_slots_map = spell_slots.read();
                let pool = spell_data.pool;
                let fname = StoredValue::new(name.to_string());
                let casting_ability = spell_data.casting_ability;
                let all_spells = spell_data
                    .spells
                    .iter()
                    .enumerate()
                    .filter(|(_, spell)| {
                        if spell.name.is_empty() {
                            return false;
                        }
                        if spell.level == 0 {
                            return true;
                        }
                        // Show if has remaining free uses (cost per cast)
                        if spell
                            .free_uses
                            .as_ref()
                            .is_some_and(|fu| fu.available() >= spell.cost.max(1))
                        {
                            return true;
                        }
                        if CastOption::slot_options(&spell_slots_map, spell.level, pool)
                            .next()
                            .is_some()
                        {
                            return true;
                        }
                        // Show ritual spells even without available slots
                        registry.with_spells_index(|index| {
                            index
                                .get(spell.name.as_str())
                                .is_some_and(|def| def.ritual)
                        })
                    })
                    .map(|(spell_idx, spell)| {
                        let level_str = if spell.level == 0 {
                            tr!("session-cantrips")
                        } else {
                            tr!("slot-level", {"level" => spell.level})
                        };

                        let meta_badges: Vec<AnyView> = registry.with_spells_index(|index| {
                            index.get(spell.name.as_str()).map(|sd| {
                                            let mut badges: Vec<AnyView> = Vec::new();
                                            // Cast time (non-Action only)
                                            match sd.cast_time {
                                                CastTime::Action(ActionType::BonusAction) => {
                                                    badges.push(view! {
                                                        <span class="entry-badge" title=move_tr!("action-type-bonus-action")>
                                                            <Icon name="zap" />
                                                        </span>
                                                    }.into_any());
                                                }
                                                CastTime::Action(ActionType::Reaction) => {
                                                    badges.push(view! {
                                                        <span class="entry-badge" title=move_tr!("action-type-reaction")>
                                                            <Icon name="shield" />
                                                        </span>
                                                    }.into_any());
                                                }
                                                CastTime::Rounds(rounds) => {
                                                    let label = format_rounds(rounds);
                                                    badges.push(view! {
                                                        <span class="entry-badge" title=move_tr!("ref-spell-cast-time")>
                                                            <Icon name="clock" />{label}
                                                        </span>
                                                    }.into_any());
                                                }
                                                CastTime::Action(ActionType::Action) => {}
                                            }
                                            // Range
                                            match sd.effect_range() {
                                                Some(EffectRange::Caster) => {}
                                                Some(EffectRange::Touch) => {
                                                    badges.push(view! {
                                                        <span class="entry-badge" title=move_tr!("ref-spell-range-touch")>
                                                            <Icon name="hand" />
                                                        </span>
                                                    }.into_any());
                                                }
                                                Some(EffectRange::Feet(feet)) => {
                                                    badges.push(view! {
                                                        <span class="entry-badge" title=move_tr!("ref-spell-range")>
                                                            <Icon name="ruler" />{feet}
                                                        </span>
                                                    }.into_any());
                                                }
                                                None => {}
                                            }
                                            // Duration (skip Instant)
                                            match sd.effect_duration() {
                                                Some(EffectDuration::Rounds(rounds)) => {
                                                    let label = format_rounds(rounds);
                                                    badges.push(view! {
                                                        <span class="entry-badge" title=move_tr!("ref-spell-duration")>
                                                            <Icon name="hourglass" />{label}
                                                        </span>
                                                    }.into_any());
                                                }
                                                Some(EffectDuration::Forever) => {
                                                    badges.push(view! {
                                                        <span class="entry-badge" title=move_tr!("ref-spell-duration-forever")>
                                                            <Icon name="infinity" />
                                                        </span>
                                                    }.into_any());
                                                }
                                                Some(EffectDuration::Instant) | None => {}
                                            }
                                            // Concentration
                                            if sd.concentration {
                                                badges.push(view! {
                                                    <span class="entry-badge" title=move_tr!("ref-spell-concentration")>
                                                        <Icon name="crosshair" />
                                                    </span>
                                                }.into_any());
                                            }
                                            // Components (V/S/M)
                                            if sd.components.verbal {
                                                badges.push(view! {
                                                    <span class="entry-badge" title=move_tr!("ref-spell-comp-verbal")>
                                                        <Icon name="audio-lines" />
                                                    </span>
                                                }.into_any());
                                            }
                                            if sd.components.somatic {
                                                badges.push(view! {
                                                    <span class="entry-badge" title=move_tr!("ref-spell-comp-somatic")>
                                                        <Icon name="hand-helping" />
                                                    </span>
                                                }.into_any());
                                            }
                                            if let Some(material) = &sd.components.material {
                                                let icon = if material.consumable { "gem" } else { "stone" };
                                                let label = if material.consumable {
                                                    tr!("ref-spell-comp-consumable")
                                                } else {
                                                    tr!("ref-spell-comp-material")
                                                };
                                                let title = if material.name.is_empty() {
                                                    label
                                                } else {
                                                    format!("{label}: {}", material.name)
                                                };
                                                badges.push(view! {
                                                    <span class="entry-badge" title=title>
                                                        <Icon name=icon />
                                                    </span>
                                                }.into_any());
                                            }
                                            // // Ritual
                                            // if sd.ritual {
                                            //     badges.push(view! {
                                            //         <span class="entry-badge" title=move_tr!("ref-spell-ritual")>
                                            //             <Icon name="book-open" />
                                            //         </span>
                                            //     }.into_any());
                                            // }
                                            badges
                            })
                            .unwrap_or_default()
                        });

                        let free_uses_badge = spell.free_uses.as_ref().map(|fu| {
                            let avail = fu.available();
                            let max = fu.max;
                            view! { <FreeUsesBadge available=avail max=max /> }
                        });
                        let has_free_uses = spell.free_uses.is_some();
                        let can_free_cast = spell
                            .free_uses
                            .as_ref()
                            .is_some_and(|fu| fu.available() >= spell.cost.max(1));
                        let show_cost = (has_cost_field && spell.cost > 0)
                            || (has_free_uses && spell.cost >= 2);
                        let cost_badge = show_cost.then(|| {
                            let cost = spell.cost;
                            let suffix = cost_short.clone();
                            view! {
                                <span class="entry-badge">
                                    {cost} " " {suffix}
                                </span>
                            }
                        });

                        // Build cast options: free use, points cost, slot levels
                        let spell_cost = spell.cost;

                        let mut cast_options: Vec<CastOption> = Vec::new();

                        // Free use option
                        if can_free_cast {
                            let fu = spell.free_uses.as_ref().unwrap();
                            cast_options.push(CastOption::FreeUse {
                                available: fu.available(),
                                max: fu.max,
                            });
                        }

                        // Points cost option
                        if has_cost_field && spell_cost > 0 {
                            let can_afford = entry.fields.iter().any(|field| {
                                cost_field_name.with_value(|cost_name| field.name == *cost_name)
                                    && field
                                        .value
                                        .available_points()
                                        .is_some_and(|avail| avail >= spell_cost)
                            });
                            if can_afford {
                                cast_options.push(CastOption::PointsCost {
                                    cost: spell_cost,
                                    suffix: cost_short.clone(),
                                });
                            }
                        }

                        // Slot level options (either pool — PHB: slots are
                        // interchangeable across Spellcasting and Pact Magic)
                        if spell.level > 0 {
                            cast_options.extend(CastOption::slot_options(
                                &spell_slots_map,
                                spell.level,
                                pool,
                            ));
                        }

                        // Single registry lookup yields all flags this spell-row needs
                        // (ritual gates the cast option below; concentration is read in
                        // the cast handler that fires later).
                        let (is_ritual, is_concentration) =
                            registry.with_spells_index(|index| {
                                index
                                    .get(spell.name.as_str())
                                    .map(|def| (def.ritual, def.concentration))
                                    .unwrap_or_default()
                            });
                        if spell.level > 0 && is_ritual {
                            cast_options.push(CastOption::Ritual {
                                level: spell.level,
                            });
                        }

                        let spell_name = StoredValue::new(spell.name.clone());
                        let spell_level = spell.level;
                        let can_cast = !cast_options.is_empty();
                        let cast_button = (can_cast || spell.level == 0).then(|| {
                            view! {
                                <CastButton
                                    options=cast_options
                                    on_cast=Callback::new(move |opt: CastOption| {
                                        // Open effects calculator (before deducting — we need the original state for display)
                                        fname.with_value(|key| {
                                            spell_name.with_value(|sname| {
                                                open_calc(sname, spell_level, key, pool, casting_ability, &opt);
                                                if is_concentration {
                                                    store.core().combat().concentrating().set(Some(sname.to_string()));
                                                }
                                            });
                                        });

                                        // Deduct resources (Ritual consumes nothing)
                                        match opt {
                                            CastOption::Ritual { .. } => {}
                                            CastOption::FreeUse { .. } => {
                                                fname.with_value(|key| {
                                                    feature_data.update(|map| {
                                                        if let Some(spell) = map.get_mut(key.as_str())
                                                            .and_then(|e| e.spells.as_mut())
                                                            .and_then(|sc| sc.spells.get_mut(spell_idx))
                                                            && let Some(fu) = &mut spell.free_uses
                                                        {
                                                            fu.used = fu
                                                                .used
                                                                .saturating_add(spell.cost.max(1))
                                                                .min(fu.max);
                                                        }
                                                    });
                                                });
                                            }
                                            CastOption::PointsCost { .. } => {
                                                fname.with_value(|key| {
                                                    cost_field_name.with_value(|cost_name| {
                                                        feature_data.update(|map| {
                                                            if let Some(entry) = map.get_mut(key.as_str())
                                                                && let Some(field) = entry.fields.iter_mut().find(|f| f.name == *cost_name)
                                                                && let FeatureValue::Points { used, max } = &mut field.value
                                                            {
                                                                *used = (*used + spell_cost).min(*max);
                                                            }
                                                        });
                                                    });
                                                });
                                            }
                                            CastOption::SpellSlot { pool: slot_pool, level: slot_level, .. } => {
                                                spell_slots.update(|pools| {
                                                    if let Some(slots) = pools.get_mut(&slot_pool) {
                                                        let idx = (slot_level - 1) as usize;
                                                        if let Some(slot) = slots.get_mut(idx) {
                                                            slot.used = slot.used.saturating_add(1).min(slot.total);
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                    })
                                />
                            }
                            .into_any()
                        });

                        let badge = view! {
                            <>
                                <span class="entry-badge">{level_str}</span>
                                {meta_badges}
                                {free_uses_badge}
                                {cost_badge}
                            </>
                        }
                        .into_any();

                        SessionListItem {
                            name: spell.label().to_string(),
                            description: spell.description.clone(),
                            badge: Some(badge),
                            actions: cast_button,
                            name_prefix: None,
                            name_extra: None,
                            description_view: None,
                        }
                    })
                    .collect::<Vec<_>>();

                if all_spells.is_empty() {
                    return None;
                }

                let ability = spell_data.casting_ability;
                let save_dc = eff.spell_save_dc(ability, name);
                let atk_bonus = eff.spell_attack_bonus(ability, name);
                let atk_str = format_bonus(atk_bonus);
                let atk_adv = eff.spell_attack_advantage(name);

                Some(view! {
                    <div class="session-subsection">
                        <h4 class="session-subsection-title">{feature_label}</h4>
                        <div class="session-spell-stats">
                            <span class="session-spell-stat">
                                {move_tr!("spell-save-dc")} ": " <strong>{save_dc}</strong>
                            </span>
                            <span class="session-spell-stat">
                                {move_tr!("spell-attack")} ": " <strong>{atk_str}</strong>
                                {adv_icon(atk_adv)}
                            </span>
                        </div>
                        <SessionList items=all_spells />
                    </div>
                })
            })
            .collect_view()
    };

    view! {
        {spells_view}
        <EffectsCalcModal show=show_calc info=calc_info />
    }
}
