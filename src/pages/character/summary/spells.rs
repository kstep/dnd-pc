use leptos::prelude::*;
use leptos_fluent::{move_tr, tr};
use reactive_stores::Store;

use crate::{
    components::{
        cast_button::CastButton,
        summary_list::{SummaryList, SummaryListItem},
    },
    model::{Character, CharacterStoreFields, FeatureValue, SpellSlotLevel, format_bonus},
    rules::RulesRegistry,
};

#[component]
pub fn SpellsBlock() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let store = expect_context::<Store<Character>>();
    let abilities = store.abilities();
    let identity = store.identity();
    let spell_slots = store.spell_slots();
    let feature_data = store.feature_data();

    move || {
        let prof_bonus = store.read().proficiency_bonus();

        feature_data
            .read()
            .iter()
            .filter_map(|(name, entry)| {
                let spell_data = entry.spells.as_ref()?;
                let ability = spell_data.casting_ability;

                let ability_mod = abilities.read().modifier(ability);
                let save_dc = 8 + prof_bonus + ability_mod;
                let atk_bonus = prof_bonus + ability_mod;

                let feature_label = registry
                    .with_feature(&identity.read(), name, |f| f.label().to_string())
                    .unwrap_or_else(|| name.clone());

                // Resolve cost field name and short suffix (e.g. "Sorcery Points" / "SP")
                let (cost_field_name, cost_short) = registry
                    .with_feature(&identity.read(), name, |feat| {
                        let cost_field_name = feat.spells.as_ref()?.cost.clone()?;
                        let field_def = feat.fields.get(&cost_field_name)?;
                        let short = match &field_def.kind {
                            crate::rules::FieldKind::Points { short, .. } => short.clone()?,
                            _ => return None,
                        };
                        Some((cost_field_name, short))
                    })
                    .flatten()
                    .unwrap_or_default();
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

                        // Check if points cost can be paid
                        let spell_cost = spell.cost;
                        let can_points_cast = has_cost_field
                            && spell_cost > 0
                            && entry.fields.iter().any(|f| {
                                cost_field_name.with_value(|cfn| f.name == *cfn)
                                    && f.value
                                        .available_points()
                                        .is_some_and(|avail| avail >= spell_cost)
                            });

                        // Available spell slots for leveled spells
                        let available_slots: Vec<(u32, u32)> = if spell.level > 0 {
                            (spell.level..=9)
                                .filter_map(|sl| {
                                    let idx = (sl - 1) as usize;
                                    let remaining = pool_slots
                                        .and_then(|slots| slots.get(idx))
                                        .map(|s| s.available())
                                        .unwrap_or(0);
                                    (remaining > 0).then_some((sl, remaining))
                                })
                                .collect()
                        } else {
                            Vec::new()
                        };
                        let has_slot_cast = !available_slots.is_empty();

                        // Single unified cast button, priority:
                        // 1. free uses available → spend free uses directly
                        // 2. points cost payable → spend points directly
                        // 3. spell slots available → show slot picker
                        let can_cast_directly = can_free_cast || can_points_cast;
                        let can_cast = can_cast_directly || has_slot_cast;
                        let cast_button = (spell.level > 0 && can_cast).then(|| {
                            let spell_level = spell.level;
                            view! {
                                <CastButton
                                    disabled=!can_cast
                                    slots=if can_cast_directly { Vec::new() } else { available_slots }
                                    spell_level=spell_level
                                    on_cast=move || {
                                        fname.with_value(|key| {
                                            feature_data.update(|map| {
                                                if let Some(entry) = map.get_mut(key) {
                                                    // Try free uses first
                                                    if let Some(spell) = entry.spells.as_mut()
                                                        .and_then(|sc| sc.spells.get_mut(spell_idx))
                                                        && let Some(fu) = &mut spell.free_uses
                                                        && fu.available() >= spell.cost.max(1)
                                                    {
                                                        fu.used = fu
                                                            .used
                                                            .saturating_add(spell.cost.max(1))
                                                            .min(fu.max);
                                                        return;
                                                    }
                                                    // Then try points cost
                                                    cost_field_name.with_value(|cfn| {
                                                        if !cfn.is_empty()
                                                            && let Some(field) = entry.fields.iter_mut().find(|f| f.name == *cfn)
                                                            && let FeatureValue::Points { used, max } = &mut field.value
                                                        {
                                                            *used = (*used + spell_cost).min(*max);
                                                        }
                                                    });
                                                }
                                            });
                                        });
                                    }
                                    on_slot_cast=Callback::new(move |level: u32| {
                                        spell_slots.update(|pools| {
                                            if let Some(slots) = pools.get_mut(&pool) {
                                                let idx = (level - 1) as usize;
                                                if let Some(slot) = slots.get_mut(idx) {
                                                    slot.used = slot.used.saturating_add(1).min(slot.total);
                                                }
                                            }
                                        });
                                    })
                                />
                            }
                        });

                        let has_extra =
                            has_free_uses || show_cost || can_cast;
                        let badge = view! {
                            <span class="summary-spell-badge">
                                <span class="summary-spell-level"
                                    class:has-extra=has_extra
                                >{level_str}</span>
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
