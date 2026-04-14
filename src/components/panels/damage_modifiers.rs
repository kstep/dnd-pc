use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::{icon::Icon, slot_box::SlotBox},
    model::{Character, CharacterStoreFields, DamageModifiers, DamageType, Translatable},
};

#[component]
fn DamageToggle(
    icon: &'static str,
    title: Signal<String>,
    active: Memo<bool>,
    on_toggle: impl Fn() + 'static,
) -> impl IntoView {
    view! {
        <label class="damage-toggle" title=move || title.get()>
            <input
                type="checkbox"
                prop:checked=move || active.get()
                on:change=move |_| on_toggle()
            />
            <Icon name=icon />
        </label>
    }
}

#[component]
pub fn DamageModifiersPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let i18n = expect_context::<leptos_fluent::I18n>();
    let dmg_expanded = RwSignal::new(false);

    view! {
        <section>
            <div class="section-header">
                <button
                    class="btn-toggle-desc"
                    class:expanded=move || dmg_expanded.get()
                    on:click=move |_| dmg_expanded.update(|expanded| *expanded = !*expanded)
                />
                <h3>{move_tr!("panel-damage-modifiers")}</h3>
            </div>
            <div class="slot-box-list">
            {move || {
                let expanded = dmg_expanded.get();
                DamageType::iter()
                    .filter(move |damage_type| {
                        expanded
                            || store
                                .damage_modifiers()
                                .read()
                                .get(damage_type)
                                .copied()
                                .unwrap_or_default()
                                .is_active()
                    })
                    .map(|damage_type| {
                        let current = Memo::new(move |_| {
                            store
                                .damage_modifiers()
                                .read()
                                .get(&damage_type)
                                .copied()
                                .unwrap_or_default()
                        });
                        let tr_key = damage_type.tr_key();
                        let label = Signal::derive(move || i18n.tr(tr_key));
                        let icon = damage_type.icon_name();

                        let toggle_field = move |field: fn(&mut DamageModifiers) -> &mut bool| {
                            store.damage_modifiers().update(|damage_modifiers| {
                                let entry = damage_modifiers.entry(damage_type).or_default();
                                let flag = field(entry);
                                *flag = !*flag;
                                if !entry.is_active() {
                                    damage_modifiers.remove(&damage_type);
                                }
                            });
                        };

                        view! {
                            <SlotBox label=label icon=icon>
                                <DamageToggle
                                    icon="shield-half"
                                    title=Signal::derive(move || i18n.tr("damage-resistance"))
                                    active=Memo::new(move |_| current.get().resistant)
                                    on_toggle=move || toggle_field(|modifiers| &mut modifiers.resistant)
                                />
                                <DamageToggle
                                    icon="shield-off"
                                    title=Signal::derive(move || i18n.tr("damage-vulnerability"))
                                    active=Memo::new(move |_| current.get().vulnerable)
                                    on_toggle=move || toggle_field(|modifiers| &mut modifiers.vulnerable)
                                />
                                <DamageToggle
                                    icon="shield-check"
                                    title=Signal::derive(move || i18n.tr("damage-immunity"))
                                    active=Memo::new(move |_| current.get().immune)
                                    on_toggle=move || toggle_field(|modifiers| &mut modifiers.immune)
                                />
                                <Icon name="shield-minus" />
                                <input
                                    type="number"
                                    min="0"
                                    prop:value=move || current.get().reduction
                                    on:input=move |event| {
                                        let value = event_target_value(&event)
                                            .parse::<u32>()
                                            .unwrap_or(0);
                                        store.damage_modifiers().update(|damage_modifiers| {
                                            let entry = damage_modifiers.entry(damage_type).or_default();
                                            entry.reduction = value;
                                            if !entry.is_active() {
                                                damage_modifiers.remove(&damage_type);
                                            }
                                        });
                                    }
                                />
                            </SlotBox>
                        }
                    })
                    .collect_view()
            }}
            </div>
        </section>
    }
}
