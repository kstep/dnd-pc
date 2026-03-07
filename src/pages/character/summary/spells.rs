use leptos::prelude::*;
use leptos_fluent::{move_tr, tr};
use reactive_stores::Store;

use crate::{
    components::summary_list::{SummaryList, SummaryListItem},
    model::{Character, CharacterStoreFields, SpellSlotLevel, format_bonus},
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
                                .is_some_and(SpellSlotLevel::is_available)
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
