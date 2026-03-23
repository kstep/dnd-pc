use std::collections::BTreeMap;

use leptos::{html, prelude::*};
use leptos_fluent::{move_tr, tr};
use reactive_stores::Store;

use crate::{
    components::{
        cast_button::{CastButton, CastOption},
        modal::Modal,
        summary::adv_icon,
        summary_list::{SummaryList, SummaryListItem},
    },
    effective::EffectiveCharacter,
    expr::{self, DicePool, Eval},
    model::{
        Ability, Attribute, Character, CharacterStoreFields, EffectDefinition, FeatureValue,
        SpellSlotLevel, SpellSlotPool, format_bonus,
    },
    rules::RulesRegistry,
};

// --- Read-only context for spell effect calculation ---

struct SpellCalcContext<'a> {
    character: &'a Character,
    slot_level: i32,
    caster_level: i32,
    caster_modifier: i32,
}

impl expr::Context<Attribute, i32> for SpellCalcContext<'_> {
    fn assign(&mut self, var: Attribute, _value: i32) -> Result<(), expr::Error> {
        Err(expr::Error::read_only_var(var))
    }

    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        match var {
            Attribute::SlotLevel => Ok(self.slot_level),
            Attribute::CasterLevel(None) => Ok(self.caster_level),
            Attribute::CasterModifier => Ok(self.caster_modifier),
            _ => self.character.resolve(var),
        }
    }
}

// --- Spell calc info passed to the modal ---

struct SpellCalcInfo {
    spell_label: String,
    effects: Vec<EffectDefinition>,
    slot_level: u32,
    casting_ability: Ability,
    caster_level: u32,
}

// --- Effects calculator modal ---

#[component]
fn SpellEffectsModal(
    show: RwSignal<bool>,
    info: StoredValue<Option<SpellCalcInfo>>,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let title = Signal::derive(move || {
        info.with_value(|info| {
            info.as_ref()
                .map(|i| {
                    if i.slot_level > 0 {
                        format!(
                            "{} ({})",
                            i.spell_label,
                            tr!("slot-level", {"level" => i.slot_level.to_string()})
                        )
                    } else {
                        i.spell_label.clone()
                    }
                })
                .unwrap_or_default()
        })
    });

    // Build effect views when modal opens
    let content = move || {
        if !show.get() {
            return None;
        }

        info.with_value(|info| {
            let info = info.as_ref()?;
            let character = store.read();

            let ctx = SpellCalcContext {
                character: &character,
                slot_level: info.slot_level as i32,
                caster_level: info.caster_level as i32,
                caster_modifier: character.ability_modifier(info.casting_ability),
            };

            let effect_views = info
                .effects
                .iter()
                .map(|effect| {
                    let formula_str = format!("{}", effect.expr);
                    let label = effect.label().to_string();
                    let rolls = effect.expr.dice_rolls(&ctx);

                    if rolls.is_empty() {
                        // No dice — evaluate immediately
                        let result = effect.expr.eval(&ctx).ok();
                        view! {
                            <div class="spell-effect-row">
                                <div class="spell-effect-header">
                                    <span class="spell-effect-label">{label}</span>
                                    <code class="spell-effect-formula">{formula_str}</code>
                                </div>
                                <div class="spell-effect-result">
                                    <strong>{result.map(|v| v.to_string()).unwrap_or_default()}</strong>
                                </div>
                            </div>
                        }
                        .into_any()
                    } else {
                        // Has dice — build inputs and live result
                        let result = RwSignal::new(None::<i32>);
                        let expr = effect.expr.clone();
                        let slot_level = info.slot_level;
                        let caster_level = info.caster_level;
                        let casting_ability = info.casting_ability;

                        // Create NodeRef groups per die type
                        let groups: BTreeMap<u32, Vec<NodeRef<html::Input>>> = rolls
                            .iter()
                            .map(|(&sides, &count)| {
                                let refs: Vec<_> =
                                    (0..count).map(|_| NodeRef::<html::Input>::new()).collect();
                                (sides, refs)
                            })
                            .collect();
                        let groups = StoredValue::new(groups);

                        let total_needed: u32 = rolls.values().copied().sum();
                        let recalc = StoredValue::new(move || {
                            let character = store.read_untracked();
                            let mut ctx = SpellCalcContext {
                                character: &character,
                                slot_level: slot_level as i32,
                                caster_level: caster_level as i32,
                                caster_modifier: character.ability_modifier(casting_ability),
                            };

                            let pool_map = groups.with_value(|groups| {
                                groups
                                    .iter()
                                    .map(|(&sides, refs)| {
                                        let values: Vec<u32> = refs
                                            .iter()
                                            .filter_map(|r| {
                                                r.get().and_then(|el| el.value().parse().ok())
                                            })
                                            .collect();
                                        (sides, values)
                                    })
                                    .collect::<BTreeMap<u32, Vec<u32>>>()
                            });

                            // Only evaluate if all inputs are filled
                            let total_filled: u32 =
                                pool_map.values().map(|v| v.len() as u32).sum();

                            if total_filled == total_needed {
                                let pool: DicePool = pool_map.into();
                                result.set(expr.apply_with_dice(&mut ctx, &pool).ok());
                            } else {
                                result.set(None);
                            }
                        });

                        // Build grouped input views
                        let mut first_input = true;
                        let group_views = groups.with_value(|groups| {
                            groups
                                .iter()
                                .map(|(&sides, refs)| {
                                    let input_views = refs
                                        .iter()
                                        .map(|&node_ref| {
                                            let is_first = first_input;
                                            first_input = false;
                                            view! {
                                                <input
                                                    type="number"
                                                    min=1
                                                    max=sides
                                                    autofocus=is_first
                                                    class="dice-pool-value"
                                                    node_ref=node_ref
                                                    on:input=move |_| recalc.with_value(|f| f())
                                                />
                                            }
                                        })
                                        .collect_view();
                                    view! {
                                        <div class="dice-pool-group">
                                            <span class="dice-pool-label">"d" {sides}</span>
                                            <div class="dice-pool-inputs">{input_views}</div>
                                        </div>
                                    }
                                })
                                .collect_view()
                        });

                        view! {
                            <div class="spell-effect-row">
                                <div class="spell-effect-header">
                                    <span class="spell-effect-label">{label}</span>
                                    <code class="spell-effect-formula">{formula_str}</code>
                                </div>
                                <div class="dice-pool-groups">{group_views}</div>
                                <div class="spell-effect-result">
                                    {move || result.get().map(|v| view! {
                                        <strong>{v}</strong>
                                    })}
                                </div>
                            </div>
                        }
                        .into_any()
                    }
                })
                .collect_view();

            Some(effect_views)
        })
    };

    view! {
        <Modal show=show title=title>
            <div class="spell-effects-calc">
                {content}
            </div>
        </Modal>
    }
}

#[component]
pub fn SpellsBlock() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let store = expect_context::<Store<Character>>();
    let eff = expect_context::<EffectiveCharacter>();
    let spell_slots = store.spell_slots();
    let feature_data = store.feature_data();

    // Modal state
    let show_calc = RwSignal::new(false);
    let calc_info = StoredValue::new(None::<SpellCalcInfo>);

    let open_calc = move |spell_name: &str,
                          spell_level: u32,
                          fname: &str,
                          pool: SpellSlotPool,
                          casting_ability: Ability,
                          opt: &CastOption| {
        let slot_level = match opt {
            CastOption::SpellSlot { level, .. } => *level,
            _ => spell_level,
        };

        let effects = registry
            .with_feature(fname, |feat| {
                feat.spells.as_ref().map(|spells_def| {
                    registry.with_spell_list(&spells_def.list, |spell_map| {
                        spell_map
                            .get(spell_name)
                            .map(|sd| sd.effects.clone())
                            .unwrap_or_default()
                    })
                })
            })
            .flatten()
            .unwrap_or_default();

        if !effects.is_empty() {
            let character = store.read_untracked();
            let caster_level = character.caster_level(pool);
            let spell_label = character
                .feature_data
                .get(fname)
                .and_then(|e| e.spells.as_ref())
                .and_then(|sd| sd.spells.iter().find(|s| s.name == spell_name))
                .map(|s| s.label().to_string())
                .unwrap_or_else(|| spell_name.to_string());

            calc_info.set_value(Some(SpellCalcInfo {
                spell_label,
                effects,
                slot_level,
                casting_ability,
                caster_level,
            }));
            show_calc.set(true);
        }
    };

    let spells_view = move || {
        feature_data
            .read()
            .iter()
            .filter_map(|(name, entry)| {
                let spell_data = entry.spells.as_ref()?;

                let (feature_label, cost_field_name, cost_short) = registry
                    .with_feature(name, |feat| {
                        let label = feat.label().to_string();
                        let (cost_name, cost_short) = feat
                            .cost_info()
                            .map(|(name, short)| (name.to_string(), short))
                            .unwrap_or_default();
                        (label, cost_name, cost_short)
                    })
                    .unwrap_or_else(|| (name.clone(), String::new(), String::new()));
                let has_cost_field = !cost_short.is_empty();
                let cost_field_name = StoredValue::new(cost_field_name);

                let spell_slots_map = spell_slots.read();
                let pool = spell_data.pool;
                let pool_slots = spell_slots_map.get(&pool);
                let fname = StoredValue::new(name.clone());
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
                        (spell.level..=9).any(|sl| {
                            let idx = (sl - 1) as usize;
                            pool_slots
                                .and_then(|slots| slots.get(idx))
                                .is_some_and(SpellSlotLevel::is_available)
                        })
                    })
                    .map(|(spell_idx, spell)| {
                        let level_str = if spell.level == 0 {
                            tr!("summary-cantrips")
                        } else {
                            tr!("slot-level", {"level" => spell.level.to_string()})
                        };

                        let free_uses_badge = spell.free_uses.as_ref().map(|fu| {
                            let avail = fu.available();
                            let max = fu.max;
                            view! {
                                <span class="entry-badge">
                                    {avail} "/" {max}
                                </span>
                            }
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

                        // Slot level options
                        if spell.level > 0 {
                            for sl in spell.level..=9 {
                                let idx = (sl - 1) as usize;
                                let remaining = pool_slots
                                    .and_then(|slots| slots.get(idx))
                                    .map(|slot| slot.available())
                                    .unwrap_or(0);
                                if remaining > 0 {
                                    cast_options.push(CastOption::SpellSlot {
                                        level: sl,
                                        remaining,
                                        natural: sl == spell.level,
                                    });
                                }
                            }
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
                                            });
                                        });

                                        // Deduct resources
                                        match opt {
                                            CastOption::FreeUse { .. } => {
                                                fname.with_value(|key| {
                                                    feature_data.update(|map| {
                                                        if let Some(spell) = map.get_mut(key)
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
                                                            if let Some(entry) = map.get_mut(key)
                                                                && let Some(field) = entry.fields.iter_mut().find(|f| f.name == *cost_name)
                                                                && let FeatureValue::Points { used, max } = &mut field.value
                                                            {
                                                                *used = (*used + spell_cost).min(*max);
                                                            }
                                                        });
                                                    });
                                                });
                                            }
                                            CastOption::SpellSlot { level: slot_level, .. } => {
                                                spell_slots.update(|pools| {
                                                    if let Some(slots) = pools.get_mut(&pool) {
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
                        });

                        let badge = view! {
                            <span class="entry-badge">
                                <span class="summary-spell-level">{level_str}</span>
                                {free_uses_badge}
                                {cost_badge}
                                {cast_button}
                            </span>
                        }
                        .into_any();

                        SummaryListItem {
                            name: spell.label().to_string(),
                            description: spell.description.clone(),
                            badge: Some(badge),
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
                    <div class="summary-subsection">
                        <h4 class="summary-subsection-title">{feature_label}</h4>
                        <div class="summary-spell-stats">
                            <span class="summary-spell-stat">
                                {move_tr!("spell-save-dc")} ": " <strong>{save_dc}</strong>
                            </span>
                            <span class="summary-spell-stat">
                                {move_tr!("spell-attack")} ": " <strong>{atk_str}</strong>
                                {adv_icon(atk_adv)}
                            </span>
                        </div>
                        <SummaryList items=all_spells />
                    </div>
                })
            })
            .collect_view()
    };

    view! {
        {spells_view}
        <SpellEffectsModal show=show_calc info=calc_info />
    }
}
