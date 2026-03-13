use leptos::prelude::*;
use leptos_fluent::{move_tr, tr};
use reactive_stores::Store;

use crate::{
    components::{
        cast_button::{CastButton, CastOption},
        summary_list::{SummaryList, SummaryListItem},
    },
    effective::EffectiveCharacter,
    model::{Character, CharacterStoreFields, FeatureValue, SpellSlotLevel, format_bonus},
    rules::RulesRegistry,
};

#[component]
pub fn SpellsBlock() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let store = expect_context::<Store<Character>>();
    let eff = expect_context::<EffectiveCharacter>();
    let identity = store.identity();
    let spell_slots = store.spell_slots();
    let feature_data = store.feature_data();

    move || {
        let id = identity.read();
        feature_data
            .read()
            .iter()
            .filter_map(|(name, entry)| {
                let spell_data = entry.spells.as_ref()?;
                let ability = spell_data.casting_ability;

                let save_dc = eff.spell_save_dc(ability);
                let atk_bonus = eff.spell_attack_bonus(ability);

                let (feature_label, cost_field_name, cost_short) = registry
                    .with_feature(&id, name, |feat| {
                        let label = feat.label().to_string();
                        let (cost_name, cost_short) = feat
                            .cost_info()
                            .map(|(name, short)| (name.to_string(), short.to_string()))
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
                let all_spells = spell_data
                    .spells
                    .iter()
                    .enumerate()
                    .filter(|(_, spell)| {
                        if spell.name.is_empty() {
                            return false;
                        }
                        if spell.level == 0 {
                            return spell.prepared || spell.sticky;
                        }
                        if !spell.prepared && !spell.sticky {
                            return false;
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
                                <span class="summary-list-badge">
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
                                <span class="summary-list-badge">
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

                        let can_cast = !cast_options.is_empty();
                        let cast_button = (spell.level > 0 && can_cast).then(|| {
                            view! {
                                <CastButton
                                    options=cast_options
                                    on_cast=Callback::new(move |opt: CastOption| {
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
                            <span class="summary-spell-badge">
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

                let atk_str = format_bonus(atk_bonus);

                Some(view! {
                    <div class="summary-subsection">
                        <h4 class="summary-subsection-title">{feature_label}</h4>
                        <div class="summary-spell-stats">
                            <span class="summary-spell-stat">
                                {move_tr!("spell-save-dc")} ": " <strong>{save_dc}</strong>
                            </span>
                            <span class="summary-spell-stat">
                                {move_tr!("spell-attack")} ": " <strong>{atk_str}</strong>
                            </span>
                        </div>
                        <SummaryList items=all_spells />
                    </div>
                })
            })
            .collect_view()
    }
}
