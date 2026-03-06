use leptos::prelude::*;
use leptos_fluent::{move_tr, tr};
use reactive_stores::Store;

use crate::{
    components::summary_list::{SummaryList, SummaryListItem},
    model::{Character, CharacterStoreFields, format_bonus},
    rules::RulesRegistry,
};

#[component]
pub fn SpellsBlock() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let store = expect_context::<Store<Character>>();
    let abilities = store.abilities().get();
    let identity = store.identity();
    let spell_slots = store.spell_slots();
    let prof_bonus = store.read().proficiency_bonus();

    let spell_sections = store
        .feature_data()
        .read()
        .iter()
        .filter_map(|(name, entry)| {
            let spell_data = entry.spells.as_ref()?;
            let ability = spell_data.casting_ability;

            let ability_mod = abilities.modifier(ability);
            let save_dc = 8 + prof_bonus + ability_mod;
            let atk_bonus = prof_bonus + ability_mod;

            let feature_label = registry
                .with_feature(&identity.read(), name, |f| f.label().to_string())
                .unwrap_or_else(|| name.clone());

            let spell_slots_map = spell_slots.read();
            let pool = spell_data.pool;
            let pool_slots = spell_slots_map.get(&pool);
            let all_spells = spell_data
                .spells
                .iter()
                .filter(|spell| {
                    if spell.name.is_empty() {
                        return false;
                    }
                    if spell.level == 0 {
                        return spell.prepared || spell.sticky;
                    }
                    if !spell.prepared && !spell.sticky {
                        return false;
                    }
                    (spell.level..=9).any(|sl| {
                        let idx = (sl - 1) as usize;
                        pool_slots
                            .and_then(|slots| slots.get(idx))
                            .is_some_and(|slot| slot.total > 0 && slot.used < slot.total)
                    })
                })
                .map(|spell| {
                    let level_str = if spell.level == 0 {
                        tr!("summary-cantrips")
                    } else {
                        tr!("slot-level", {"level" => spell.level.to_string()})
                    };

                    SummaryListItem {
                        name: spell.label().to_string(),
                        description: spell.description.clone(),
                        badge: Some(
                            view! {
                                <span class="summary-list-badge">{level_str}</span>
                            }
                            .into_any(),
                        ),
                    }
                })
                .collect::<Vec<_>>();

            if all_spells.is_empty() {
                return None;
            }

            Some((feature_label, save_dc, atk_bonus, all_spells))
        })
        .collect::<Vec<_>>();

    if spell_sections.is_empty() {
        return None;
    }

    Some(
        spell_sections
            .into_iter()
            .map(|(label, dc, atk, spells)| {
                let atk_str = format_bonus(atk);
                view! {
                    <div class="summary-subsection">
                        <h4 class="summary-subsection-title">{label}</h4>
                        <div class="summary-spell-stats">
                            <span class="summary-spell-stat">
                                {move_tr!("spell-save-dc")} ": " <strong>{dc}</strong>
                            </span>
                            <span class="summary-spell-stat">
                                {move_tr!("spell-attack")} ": " <strong>{atk_str}</strong>
                            </span>
                        </div>
                        <SummaryList items=spells />
                    </div>
                }
            })
            .collect_view(),
    )
}
