use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::{icon::Icon, panel::Panel},
    model::{
        Ability, Character, CharacterStoreFields, DamageModifiers, DamageType, Translatable,
        format_bonus,
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
            <Icon name=icon size=14 />
        </label>
    }
}

#[component]
pub fn SavingThrowsPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let i18n = expect_context::<leptos_fluent::I18n>();

    view! {
        <Panel title=move_tr!("panel-saving-throws") class="saving-throws-panel">
            {Ability::iter()
                .map(|ability| {
                    let proficient = Memo::new(move |_| {
                        store.saving_throws().read().contains(&ability)
                    });
                    let bonus = Memo::new(move |_| store.read().saving_throw_bonus(ability));
                    let bonus_display = move || format_bonus(bonus.get());
                    let tr_key = ability.tr_key();
                    let label = Signal::derive(move || i18n.tr(tr_key));

                    view! {
                        <div class="save-row">
                            <button
                                class="prof-toggle"
                                on:click=move |_| {
                                    store.saving_throws().update(|saves| {
                                        if !saves.remove(&ability) {
                                            saves.insert(ability);
                                        }
                                    });
                                }
                            >
                                <Icon name=Signal::derive(move || if proficient.get() { "circle-dot" } else { "circle-dashed" }) size=14 />
                            </button>
                            <span class="save-bonus">{bonus_display}</span>
                            <span class="save-label">{label}</span>
                        </div>
                    }
                })
                .collect_view()}

            // --- Damage Modifiers ---
            <h4 class="panel-subsection-title">{move_tr!("summary-damage-modifiers")}</h4>
            {DamageType::iter()
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
                        <div class="damage-row">
                            <span class="damage-dt-icon"><Icon name=icon size=14 /></span>
                            <span class="damage-label">{label}</span>
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
                            <span class="damage-dr">
                                <Icon name="shield-minus" size=14 />
                                <input
                                    type="number"
                                    min="0"
                                    class="short-input"
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
                            </span>
                        </div>
                    }
                })
                .collect_view()}
        </Panel>
    }
}
