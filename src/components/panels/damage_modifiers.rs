use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::{icon::Icon, slot_box::SlotBox},
    model::{
        Character, CharacterCoreStoreFields, CharacterStoreFields, DamageModifier, DamageType,
        Translatable,
    },
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
                <h3
                    class="clickable"
                    on:click=move |_| dmg_expanded.update(|expanded| *expanded = !*expanded)
                >
                    {move_tr!("panel-damage-modifiers")}
                </h3>
            </div>
            <div class="slot-box-list">
            {move || {
                let expanded = dmg_expanded.get();
                DamageType::iter()
                    .filter(move |damage_type| {
                        expanded || store.core().damage_modifiers().read().get_entry(*damage_type).is_active()
                    })
                    .map(|damage_type| {
                        let current = Memo::new(move |_| {
                            store.core().damage_modifiers().read().get_entry(damage_type)
                        });
                        let tr_key = damage_type.tr_key();
                        let label = Signal::derive(move || i18n.tr(tr_key));
                        let icon = damage_type.icon_name();

                        let toggle_field = move |field: fn(&mut DamageModifier) -> &mut bool| {
                            store
                                .core().damage_modifiers()
                                .update(|dm| dm.toggle(damage_type, field));
                        };

                        view! {
                            <SlotBox label=label icon=icon>
                                <DamageToggle
                                    icon="shield-half"
                                    title=move_tr!("damage-resistance")
                                    active=Memo::new(move |_| current.get().resistant)
                                    on_toggle=move || toggle_field(|modifiers| &mut modifiers.resistant)
                                />
                                <DamageToggle
                                    icon="shield-off"
                                    title=move_tr!("damage-vulnerability")
                                    active=Memo::new(move |_| current.get().vulnerable)
                                    on_toggle=move || toggle_field(|modifiers| &mut modifiers.vulnerable)
                                />
                                <DamageToggle
                                    icon="shield-check"
                                    title=move_tr!("damage-immunity")
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
                                        store
                                            .core().damage_modifiers()
                                            .update(|dm| dm.set_reduction(damage_type, value));
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
