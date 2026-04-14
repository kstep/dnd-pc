use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{icon::Icon, slot_box::SlotBox, stat_box::StatBox},
    model::{
        Character, CharacterIdentityStoreFields, CharacterStoreFields, CombatStatsStoreFields,
        format_bonus,
    },
    rules::RulesRegistry,
};

#[component]
pub fn CombatPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let combat = store.combat();
    let classes = store.identity().classes();
    let initiative = Memo::new(move |_| store.read().initiative());

    let init_display = move || format_bonus(initiative.get());

    view! {
        <section>
            <h3>{move_tr!("panel-combat")}</h3>
            <div class="slot-box-list">
                <StatBox label=move_tr!("armor-class")>
                    <input
                        type="number"
                        prop:value=move || store.read().armor_class()
                        on:input=move |e| {
                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                combat.armor_class().set(value);
                            }
                        }
                    />
                </StatBox>
                <StatBox label=move_tr!("initiative")>
                    <span class="stat-highlight">{init_display}</span>
                </StatBox>
                <StatBox label=move_tr!("inspiration")>
                    <button
                        class="inspiration-toggle"
                        class:active=move || combat.inspiration().get()
                        on:click=move |_| {
                            combat.inspiration().update(|v| *v = !*v);
                        }
                    >
                        {move || if combat.inspiration().get() { "\u{2605}" } else { "\u{2606}" }}
                    </button>
                </StatBox>
                <StatBox label=move_tr!("speed")>
                    <input
                        type="number"
                        prop:value=move || combat.speed().get().to_string()
                        on:input=move |e| {
                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                combat.speed().set(value);
                            }
                        }
                    />
                </StatBox>
                <StatBox label=move_tr!("attack-count")>
                    <input
                        type="number"
                        min="1"
                        prop:value=move || combat.attack_count().get().to_string()
                        on:input=move |event| {
                            if let Ok(value) = event_target_value(&event).parse::<u32>() {
                                combat.attack_count().set(value.max(1));
                            }
                        }
                    />
                </StatBox>
                <StatBox label=move_tr!("hp")>
                        <input
                            type="number"
                            prop:value=move || combat.hp_current().get().to_string()
                            on:input=move |e| {
                                if let Ok(value) = event_target_value(&e).parse() {
                                    combat.hp_current().set(value);
                                }
                            }
                        />
                        <span>"/"</span>
                        <input
                            type="number"
                            prop:value=move || combat.hp_max().get().to_string()
                            on:input=move |e| {
                                if let Ok(value) = event_target_value(&e).parse() {
                                    combat.hp_max().set(value);
                                }
                            }
                        />
                    </StatBox>
                <StatBox label=move_tr!("temp-hp")>
                    <input
                        type="number"
                        prop:value=move || combat.hp_temp().get().to_string()
                        on:input=move |e| {
                            if let Ok(value) = event_target_value(&e).parse() {
                                combat.hp_temp().set(value);
                            }
                        }
                    />
                </StatBox>
            </div>

            <div class="hit-dice-section">
                <h4>{move_tr!("hit-dice")}</h4>
                {move || {
                    classes
                        .read()
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| c.level > 0)
                        .map(|(i, class)| {
                            let class_label = if class.class.is_empty() {
                                format!("Class {}", i + 1)
                            } else {
                                class.class_label().to_string()
                            };
                            let label = format!("{class_label} d{}", class.hit_die_sides);
                            let total = class.level;
                            let used_val = class.hit_dice_used;
                            view! {
                                <SlotBox label=label>
                                    <input
                                        type="number"
                                        min="0"
                                        prop:max=total
                                        prop:value=used_val
                                        on:input=move |e| {
                                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                let max = classes.read()[i].level;
                                                classes.write()[i].hit_dice_used = value.min(max);
                                            }
                                        }
                                    />
                                    " / "
                                    <input
                                        type="number"
                                        min="0"
                                        prop:value=total
                                        on:input=move |e| {
                                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                classes.write()[i].level = value;
                                            }
                                        }
                                    />
                                </SlotBox>
                            }
                        })
                        .collect_view()
                }}
            </div>

            <div class="rest-actions">
                <button
                    class="btn-rest"
                    on:click=move |_| {
                        store.update(|ch| ch.long_rest());
                    }
                >
                    <Icon name="list-restart" />
                    " "
                    {move_tr!("reset-stats")}
                </button>
                <button
                    class="btn-rest"
                    title=move_tr!("recalculate")
                    on:click=move |_| {
                        store.update(|ch| registry.compute(ch));
                    }
                >
                    <Icon name="refresh-cw" />
                    " "
                    {move_tr!("recalculate")}
                </button>
            </div>
        </section>
    }
}
