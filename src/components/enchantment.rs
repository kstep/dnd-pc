use leptos::prelude::*;
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;
use strum::VariantArray;

use crate::{
    components::{icon::Icon, modal::Modal},
    model::{
        ActionType, Charges, EffectDefinition, EffectDuration, EffectRange, Enchantment,
        EnchantmentStoreFields, Expr, Translatable,
    },
    rules::{ActionDefinition, Assignment, ChoiceOption, ChoiceOptions, WhenCondition},
};

/// `WhenCondition` variants offered for gear assigns. Feature-only timings
/// (`OnFeatureAdd`, `OnCompute`) are excluded since gear doesn't go through
/// the apply pipeline.
const GEAR_WHEN: &[WhenCondition] = &[
    WhenCondition::OnGearActive,
    WhenCondition::OnEffect,
    WhenCondition::OnLongRest,
    WhenCondition::OnShortRest,
];

/// Modal for editing an `Enchantment` block on a piece of gear. Opens with a
/// snapshot of the current value, edits a local draft, and commits via
/// `on_save` only on the Save button. Cancel discards.
#[component]
pub fn EnchantmentModal(
    show: RwSignal<bool>,
    #[prop(into)] value: Signal<Enchantment>,
    on_save: Callback<Enchantment>,
) -> impl IntoView {
    let draft = Store::new(Enchantment::default());

    Effect::new(move || {
        if show.get() {
            draft.set(value.get_untracked());
        }
    });

    let save = move |_| {
        on_save.run(draft.get_untracked());
        show.set(false);
    };

    view! {
        <Modal show title=move_tr!("enchantment-edit")>
            <div class="modal-body enchantment-modal">
                <ChargesSection draft />
                <PassivesSection draft />
                <ActionsSection draft />
            </div>
            <div class="modal-actions">
                <button on:click=move |_| show.set(false)>
                    {move_tr!("btn-cancel")}
                </button>
                <button class="btn-primary" on:click=save>
                    {move_tr!("btn-save")}
                </button>
            </div>
        </Modal>
    }
}

#[component]
fn ChargesSection(draft: Store<Enchantment>) -> impl IntoView {
    let charges = draft.charges();
    let has_charges = move || charges.read().is_some();

    let toggle = move |event: web_sys::Event| {
        let enable = event_target_checked(&event);
        charges.set(if enable {
            Some(Charges::default())
        } else {
            None
        });
    };

    view! {
        <section class="enchant-section">
            <label class="enchant-section-toggle">
                <input
                    type="checkbox"
                    prop:checked=has_charges
                    on:change=toggle
                />
                <h3>{move_tr!("enchantment-charges")}</h3>
            </label>
            <Show when=has_charges>
                <ChargesEditor charges=charges.into() />
            </Show>
        </section>
    }
}

#[component]
fn ChargesEditor(charges: reactive_stores::Field<Option<Charges>>) -> impl IntoView {
    let used = move || charges.read().as_ref().map(|c| c.used).unwrap_or_default();
    let max = move || charges.read().as_ref().map(|c| c.max).unwrap_or_default();

    let on_used = move |event: web_sys::Event| {
        let value = event_target_value(&event).parse::<u32>().unwrap_or(0);
        charges.update(|c| {
            if let Some(c) = c {
                c.used = value.min(c.max);
            }
        });
    };

    let on_max = move |event: web_sys::Event| {
        let value = event_target_value(&event).parse::<u32>().unwrap_or(0);
        charges.update(|c| {
            if let Some(c) = c {
                c.max = value;
                if c.used > c.max {
                    c.used = c.max;
                }
            }
        });
    };

    view! {
        <div class="charges-row">
            <label class="field-inline">
                {move_tr!("enchantment-charges-used")}
                <input
                    type="number"
                    class="short-input"
                    min="0"
                    prop:value=move || used().to_string()
                    on:change=on_used
                />
            </label>
            <span class="charges-sep">"/"</span>
            <label class="field-inline">
                {move_tr!("enchantment-charges-max")}
                <input
                    type="number"
                    class="short-input"
                    min="0"
                    prop:value=move || max().to_string()
                    on:change=on_max
                />
            </label>
        </div>
    }
}

#[component]
fn PassivesSection(draft: Store<Enchantment>) -> impl IntoView {
    let assigns = draft.assign();
    let i18n = expect_context::<I18n>();

    let add = move |_| {
        assigns.write().push(Assignment {
            expr: Expr::default(),
            when: WhenCondition::OnEffect,
        });
    };

    view! {
        <section class="enchant-section">
            <h3>{move_tr!("enchantment-passives")}</h3>
            {move || {
                let len = assigns.read().len();
                if len == 0 {
                    view! { <p class="entry-empty">{move_tr!("enchantment-no-passives")}</p> }
                        .into_any()
                } else {
                    view! {
                        <div class="entry-list">
                            {(0..len).map(|i| {
                                view! { <PassiveRow assigns=assigns.into() index=i i18n=i18n /> }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }
            }}
            <button class="btn-primary" on:click=add>
                {move_tr!("btn-add-passive")}
            </button>
        </section>
    }
}

#[component]
fn PassiveRow(
    assigns: reactive_stores::Field<Vec<Assignment>>,
    index: usize,
    i18n: I18n,
) -> impl IntoView {
    let expr_str = move || {
        assigns
            .read()
            .get(index)
            .map(|a| a.expr.to_string())
            .unwrap_or_default()
    };
    let current_when = move || -> WhenCondition {
        assigns
            .read()
            .get(index)
            .map(|a| a.when)
            .unwrap_or(WhenCondition::OnEffect)
    };

    let on_expr = move |event: web_sys::Event| {
        let raw = event_target_value(&event);
        let parsed: Expr = raw.parse().unwrap_or_default();
        assigns.update(|list| {
            if let Some(a) = list.get_mut(index) {
                a.expr = parsed;
            }
        });
    };

    let on_when = move |event: web_sys::Event| {
        let Ok(when) = event_target_value(&event).parse::<WhenCondition>() else {
            return;
        };
        assigns.update(|list| {
            if let Some(a) = list.get_mut(index) {
                a.when = when;
            }
        });
    };

    let remove = move |_| {
        assigns.update(|list| {
            if index < list.len() {
                list.remove(index);
            }
        });
    };

    view! {
        <div class="entry-item">
            <div class="entry-content passive-row">
                <input
                    type="text"
                    class="entry-name"
                    placeholder=move_tr!("effect-expr")
                    prop:value=expr_str
                    on:change=on_expr
                />
                <select on:change=on_when>
                    {GEAR_WHEN.iter().map(|&when| {
                        let label = Signal::derive(move || i18n.tr(when.tr_key()));
                        let selected = move || current_when() == when;
                        view! {
                            <option value=when.to_string() selected=selected>{label}</option>
                        }
                    }).collect_view()}
                </select>
            </div>
            <div class="entry-actions">
                <button class="btn-remove" on:click=remove>
                    <Icon name="x" />
                </button>
            </div>
        </div>
    }
}

#[component]
fn ActionsSection(draft: Store<Enchantment>) -> impl IntoView {
    let actions = draft.actions();

    let add = move |_| {
        actions.write().push(ActionDefinition {
            name: Box::from(""),
            options: ChoiceOptions::List(vec![ChoiceOption {
                name: Box::from(""),
                label: None,
                description: String::new(),
                cost: 0,
                consumes: 0,
                level: 0,
                action: None,
                effects: Vec::new(),
            }]),
            cost: None,
        });
    };

    view! {
        <section class="enchant-section">
            <h3>{move_tr!("enchantment-actions")}</h3>
            {move || {
                let len = actions.read().len();
                if len == 0 {
                    view! { <p class="entry-empty">{move_tr!("enchantment-no-actions")}</p> }
                        .into_any()
                } else {
                    view! {
                        <div class="entry-list">
                            {(0..len).map(|i| view! {
                                <ActionRow actions=actions.into() index=i />
                            }).collect_view()}
                        </div>
                    }.into_any()
                }
            }}
            <button class="btn-primary" on:click=add>
                {move_tr!("btn-add-action")}
            </button>
        </section>
    }
}

#[component]
fn ActionRow(
    actions: reactive_stores::Field<Vec<ActionDefinition>>,
    index: usize,
) -> impl IntoView {
    let name = move || {
        actions
            .read()
            .get(index)
            .map(|a| a.name.to_string())
            .unwrap_or_default()
    };

    let on_name = move |event: web_sys::Event| {
        let raw = event_target_value(&event);
        actions.update(|list| {
            if let Some(a) = list.get_mut(index) {
                a.name = raw.into_boxed_str();
            }
        });
    };

    let remove = move |_| {
        actions.update(|list| {
            if index < list.len() {
                list.remove(index);
            }
        });
    };

    view! {
        <div class="action-card">
            <div class="action-card-header">
                <input
                    type="text"
                    class="entry-name action-card-name"
                    placeholder=move_tr!("enchantment-action-name")
                    prop:value=name
                    on:change=on_name
                />
                <button class="btn-remove" on:click=remove>
                    <Icon name="x" />
                </button>
            </div>
            <OptionsList actions=actions index=index />
        </div>
    }
}

#[component]
fn OptionsList(
    actions: reactive_stores::Field<Vec<ActionDefinition>>,
    index: usize,
) -> impl IntoView {
    let options_count = move || {
        actions
            .read()
            .get(index)
            .and_then(|a| match &a.options {
                ChoiceOptions::List(list) => Some(list.len()),
                _ => None,
            })
            .unwrap_or(0)
    };

    let add_option = move |_| {
        actions.update(|list| {
            if let Some(a) = list.get_mut(index)
                && let ChoiceOptions::List(opts) = &mut a.options
            {
                opts.push(ChoiceOption {
                    name: Box::from(""),
                    label: None,
                    description: String::new(),
                    cost: 0,
                    consumes: 0,
                    level: 0,
                    action: None,
                    effects: Vec::new(),
                });
            }
        });
    };

    view! {
        <div class="action-options">
            {move || (0..options_count()).map(|opt_idx| view! {
                <OptionRow actions=actions index=index opt_idx=opt_idx />
            }).collect_view()}
            <button class="btn-secondary" on:click=add_option>
                {move_tr!("btn-add-option")}
            </button>
        </div>
    }
}

#[component]
fn OptionRow(
    actions: reactive_stores::Field<Vec<ActionDefinition>>,
    index: usize,
    opt_idx: usize,
) -> impl IntoView {
    let i18n = expect_context::<I18n>();

    let read_option = move |f: &dyn Fn(&ChoiceOption) -> String| -> String {
        actions
            .read()
            .get(index)
            .and_then(|a| match &a.options {
                ChoiceOptions::List(list) => list.get(opt_idx).map(f),
                _ => None,
            })
            .unwrap_or_default()
    };

    let read_action_type = move || -> Option<ActionType> {
        actions.read().get(index).and_then(|a| match &a.options {
            ChoiceOptions::List(list) => list.get(opt_idx).and_then(|opt| opt.action),
            _ => None,
        })
    };

    let update_option = move |f: Box<dyn FnOnce(&mut ChoiceOption)>| {
        actions.update(|list| {
            if let Some(a) = list.get_mut(index)
                && let ChoiceOptions::List(opts) = &mut a.options
                && let Some(opt) = opts.get_mut(opt_idx)
            {
                f(opt);
            }
        });
    };

    let name = move || read_option(&|opt| opt.name.to_string());
    let cost = move || read_option(&|opt| opt.cost.to_string());
    let consumes = move || read_option(&|opt| opt.consumes.to_string());

    let on_name = move |event: web_sys::Event| {
        let raw = event_target_value(&event);
        update_option(Box::new(move |opt| opt.name = raw.into_boxed_str()));
    };
    let on_cost = move |event: web_sys::Event| {
        let value = event_target_value(&event).parse::<u32>().unwrap_or(0);
        update_option(Box::new(move |opt| opt.cost = value));
    };
    let on_consumes = move |event: web_sys::Event| {
        let value = event_target_value(&event).parse::<u32>().unwrap_or(0);
        update_option(Box::new(move |opt| opt.consumes = value));
    };
    let on_action_type = move |event: web_sys::Event| {
        let raw = event_target_value(&event);
        let parsed = if raw.is_empty() {
            None
        } else {
            raw.parse::<ActionType>().ok()
        };
        update_option(Box::new(move |opt| opt.action = parsed));
    };

    let remove = move |_| {
        actions.update(|list| {
            if let Some(a) = list.get_mut(index)
                && let ChoiceOptions::List(opts) = &mut a.options
                && opt_idx < opts.len()
            {
                opts.remove(opt_idx);
            }
        });
    };

    let add_effect = move |_| {
        update_option(Box::new(|opt| {
            opt.effects.push(EffectDefinition {
                name: String::new(),
                label: None,
                expr: None,
                range: EffectRange::default(),
                duration: Default::default(),
                stackable: false,
                scope: None,
            });
        }));
    };

    view! {
        <div class="option-card">
            <div class="option-card-header">
                <input
                    type="text"
                    class="entry-name"
                    placeholder=move_tr!("enchantment-option-name")
                    prop:value=name
                    on:change=on_name
                />
                <button class="btn-remove" on:click=remove>
                    <Icon name="x" />
                </button>
            </div>
            <div class="option-card-meta">
                <label class="field-inline field-cost">
                    <span class="field-label">{move_tr!("choice-cost")}</span>
                    <input
                        type="number"
                        min="0"
                        prop:value=cost
                        on:change=on_cost
                    />
                </label>
                <label class="field-inline">
                    <span class="field-label">{move_tr!("choice-consumes")}</span>
                    <input
                        type="number"
                        min="0"
                        prop:value=consumes
                        on:change=on_consumes
                    />
                </label>
                <label class="field-inline">
                    <span class="field-label">{move_tr!("action-type")}</span>
                    <select on:change=on_action_type>
                        <option value="" selected=move || read_action_type().is_none()>
                            "—"
                        </option>
                        {ActionType::VARIANTS.iter().map(|&action_type| {
                            let label = Signal::derive(move || i18n.tr(action_type.tr_key()));
                            let selected = move || read_action_type() == Some(action_type);
                            view! {
                                <option value=action_type.to_string() selected=selected>{label}</option>
                            }
                        }).collect_view()}
                    </select>
                </label>
            </div>
            <div class="option-effects">
                <h5 class="option-effects-title">{move_tr!("effects")}</h5>
                <EffectsList actions=actions index=index opt_idx=opt_idx />
                <button class="btn-secondary btn-add-effect" on:click=add_effect>
                    "+ " {move_tr!("btn-add-effect")}
                </button>
            </div>
        </div>
    }
}

#[component]
fn EffectsList(
    actions: reactive_stores::Field<Vec<ActionDefinition>>,
    index: usize,
    opt_idx: usize,
) -> impl IntoView {
    let count = move || {
        actions
            .read()
            .get(index)
            .and_then(|a| match &a.options {
                ChoiceOptions::List(list) => list.get(opt_idx).map(|opt| opt.effects.len()),
                _ => None,
            })
            .unwrap_or(0)
    };

    view! {
        {move || (0..count()).map(|eff_idx| view! {
            <EffectRow actions=actions index=index opt_idx=opt_idx eff_idx=eff_idx />
        }).collect_view()}
    }
}

#[component]
fn EffectRow(
    actions: reactive_stores::Field<Vec<ActionDefinition>>,
    index: usize,
    opt_idx: usize,
    eff_idx: usize,
) -> impl IntoView {
    let read_effect = move |f: &dyn Fn(&EffectDefinition) -> String| -> String {
        actions
            .read()
            .get(index)
            .and_then(|a| match &a.options {
                ChoiceOptions::List(list) => list
                    .get(opt_idx)
                    .and_then(|opt| opt.effects.get(eff_idx))
                    .map(f),
                _ => None,
            })
            .unwrap_or_default()
    };

    let update_effect = move |f: Box<dyn FnOnce(&mut EffectDefinition)>| {
        actions.update(|list| {
            if let Some(a) = list.get_mut(index)
                && let ChoiceOptions::List(opts) = &mut a.options
                && let Some(opt) = opts.get_mut(opt_idx)
                && let Some(eff) = opt.effects.get_mut(eff_idx)
            {
                f(eff);
            }
        });
    };

    let read_effect_full = move |f: &dyn Fn(&EffectDefinition) -> EffectDefinition| {
        actions
            .read()
            .get(index)
            .and_then(|a| match &a.options {
                ChoiceOptions::List(list) => list
                    .get(opt_idx)
                    .and_then(|opt| opt.effects.get(eff_idx))
                    .map(f),
                _ => None,
            })
            .unwrap_or_else(|| EffectDefinition {
                name: String::new(),
                label: None,
                expr: None,
                range: EffectRange::default(),
                duration: EffectDuration::default(),
                stackable: false,
                scope: None,
            })
    };

    let name = move || read_effect(&|e| e.name.clone());
    let expr =
        move || read_effect(&|e| e.expr.as_ref().map(ToString::to_string).unwrap_or_default());
    let scope = move || read_effect(&|e| e.scope.clone().unwrap_or_default());
    let range = move || read_effect_full(&|e| e.clone()).range;
    let duration = move || read_effect_full(&|e| e.clone()).duration;
    let stackable = move || read_effect_full(&|e| e.clone()).stackable;

    let on_name = move |event: web_sys::Event| {
        let raw = event_target_value(&event);
        update_effect(Box::new(move |eff| eff.name = raw));
    };
    let on_expr = move |event: web_sys::Event| {
        let raw = event_target_value(&event);
        let parsed = if raw.trim().is_empty() {
            None
        } else {
            raw.parse::<Expr>().ok()
        };
        update_effect(Box::new(move |eff| eff.expr = parsed));
    };
    let on_scope = move |event: web_sys::Event| {
        let raw = event_target_value(&event);
        update_effect(Box::new(move |eff| {
            eff.scope = if raw.trim().is_empty() {
                None
            } else {
                Some(raw)
            };
        }));
    };
    let on_range_kind = move |event: web_sys::Event| {
        let idx = event_target_value(&event).parse::<u8>().unwrap_or(0);
        update_effect(Box::new(move |eff| {
            eff.range = match idx {
                0 => EffectRange::Caster,
                1 => EffectRange::Touch,
                _ => match eff.range {
                    EffectRange::Feet(n) => EffectRange::Feet(n),
                    _ => EffectRange::Feet(30),
                },
            };
        }));
    };
    let on_range_feet = move |event: web_sys::Event| {
        let value = event_target_value(&event).parse::<u32>().unwrap_or(0);
        update_effect(Box::new(move |eff| eff.range = EffectRange::Feet(value)));
    };
    let on_duration_kind = move |event: web_sys::Event| {
        let idx = event_target_value(&event).parse::<u8>().unwrap_or(0);
        update_effect(Box::new(move |eff| {
            eff.duration = match idx {
                0 => EffectDuration::Instant,
                2 => EffectDuration::Forever,
                _ => match eff.duration {
                    EffectDuration::Rounds(n) => EffectDuration::Rounds(n),
                    _ => EffectDuration::Rounds(1),
                },
            };
        }));
    };
    let on_duration_rounds = move |event: web_sys::Event| {
        let value = event_target_value(&event).parse::<u32>().unwrap_or(0);
        update_effect(Box::new(move |eff| {
            eff.duration = EffectDuration::Rounds(value)
        }));
    };
    let on_stackable = move |event: web_sys::Event| {
        let checked = event_target_checked(&event);
        update_effect(Box::new(move |eff| eff.stackable = checked));
    };

    let remove = move |_| {
        actions.update(|list| {
            if let Some(a) = list.get_mut(index)
                && let ChoiceOptions::List(opts) = &mut a.options
                && let Some(opt) = opts.get_mut(opt_idx)
                && eff_idx < opt.effects.len()
            {
                opt.effects.remove(eff_idx);
            }
        });
    };

    let range_kind_idx = move || match range() {
        EffectRange::Caster => 0u8,
        EffectRange::Touch => 1,
        EffectRange::Feet(_) => 2,
    };
    let range_feet = move || match range() {
        EffectRange::Feet(n) => n.to_string(),
        _ => "30".into(),
    };
    let is_feet = move || matches!(range(), EffectRange::Feet(_));

    let duration_kind_idx = move || match duration() {
        EffectDuration::Instant => 0u8,
        EffectDuration::Rounds(_) => 1,
        EffectDuration::Forever => 2,
    };
    let duration_rounds = move || match duration() {
        EffectDuration::Rounds(n) => n.to_string(),
        _ => "1".into(),
    };
    let is_rounds = move || matches!(duration(), EffectDuration::Rounds(_));

    view! {
        <div class="effect-card">
            <div class="effect-card-header">
                <input
                    type="text"
                    class="entry-name"
                    placeholder=move_tr!("effect-name")
                    prop:value=name
                    on:change=on_name
                />
                <button class="btn-remove" on:click=remove>
                    <Icon name="x" />
                </button>
            </div>
            <input
                type="text"
                class="effect-expr-input"
                placeholder=move_tr!("effect-expr")
                prop:value=expr
                on:change=on_expr
            />
            <div class="effect-card-meta">
                <label class="field-inline">
                    <span class="field-label">{move_tr!("effect-range")}</span>
                    <select
                        prop:value=move || range_kind_idx().to_string()
                        on:change=on_range_kind
                    >
                        <option value="0">{move_tr!("range-caster")}</option>
                        <option value="1">{move_tr!("range-touch")}</option>
                        <option value="2">{move_tr!("range-feet")}</option>
                    </select>
                    <Show when=is_feet>
                        <input
                            type="number"
                            min="0"
                            prop:value=range_feet
                            on:change=on_range_feet
                        />
                    </Show>
                </label>
                <label class="field-inline">
                    <span class="field-label">{move_tr!("effect-duration")}</span>
                    <select
                        prop:value=move || duration_kind_idx().to_string()
                        on:change=on_duration_kind
                    >
                        <option value="0">{move_tr!("duration-instant")}</option>
                        <option value="1">{move_tr!("duration-rounds")}</option>
                        <option value="2">{move_tr!("duration-forever")}</option>
                    </select>
                    <Show when=is_rounds>
                        <input
                            type="number"
                            min="0"
                            prop:value=duration_rounds
                            on:change=on_duration_rounds
                        />
                    </Show>
                </label>
                <label class="field-inline">
                    <input
                        type="checkbox"
                        prop:checked=stackable
                        on:change=on_stackable
                    />
                    {move_tr!("effect-stackable")}
                </label>
                <details class="field-advanced">
                    <summary>{move_tr!("enchantment-advanced")}</summary>
                    <label class="field-inline">
                        <span class="field-label">{move_tr!("effect-scope")}</span>
                        <input
                            type="text"
                            class="scope-input"
                            prop:value=scope
                            on:change=on_scope
                        />
                        <span class="field-hint">{move_tr!("effect-scope-hint")}</span>
                    </label>
                </details>
            </div>
        </div>
    }
}
