use std::collections::BTreeMap;

use leptos::{html, prelude::*};

use crate::{
    components::{icon::Icon, modal::Modal},
    expr::DicePool,
};

#[component]
pub fn DicePoolInput(
    rolls: BTreeMap<u32, u32>,
    show: RwSignal<bool>,
    on_confirm: impl Fn(DicePool) + 'static + Send + Sync,
) -> impl IntoView {
    let on_confirm = StoredValue::new(on_confirm);

    // Create Vec<NodeRef<html::Input>> per die type
    let groups: BTreeMap<u32, Vec<NodeRef<html::Input>>> = rolls
        .into_iter()
        .map(|(sides, count)| {
            let refs: Vec<_> = (0..count).map(|_| NodeRef::<html::Input>::new()).collect();
            (sides, refs)
        })
        .collect();

    let groups = StoredValue::new(groups);

    // Reset all fields when opened (autofocus handled by Modal)
    Effect::new(move || {
        if show.get() {
            groups.with_value(|groups| {
                for node_ref in groups.values().flatten() {
                    if let Some(input) = node_ref.get() {
                        input.set_value("");
                    }
                }
            });
        }
    });

    let form_ref = NodeRef::<html::Form>::new();

    let confirm = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        if let Some(form) = form_ref.get()
            && !form.report_validity()
        {
            return;
        }
        let pool = groups.with_value(|groups| {
            groups
                .iter()
                .map(|(&sides, refs)| {
                    let values: Vec<u32> = refs
                        .iter()
                        .map(|input_ref| {
                            input_ref
                                .get()
                                .and_then(|el| el.value().parse::<u32>().ok())
                                .unwrap_or(1)
                        })
                        .collect();
                    (sides, values)
                })
                .collect::<BTreeMap<u32, Vec<u32>>>()
        });
        on_confirm.with_value(|f| f(pool.into()));
        show.set(false);
    };

    // Build group views eagerly to avoid ownership issues
    let group_views = StoredValue::new(groups.with_value(|groups| {
        let mut first = true;
        groups
            .iter()
            .map(|(&sides, refs)| {
                let input_views = refs
                    .iter()
                    .map(|&node_ref| {
                        let is_first = first;
                        first = false;
                        view! {
                            <input
                                type="number"
                                min=1
                                max=sides
                                required
                                autofocus=is_first
                                class="dice-pool-value"
                                node_ref=node_ref
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
    }));

    view! {
        <Modal show=show title="Dice Rolls">
            <form class="dice-pool-form" on:submit=confirm node_ref=form_ref>
                <div class="dice-pool-groups">{group_views.get_value()}</div>
                <div class="dice-pool-footer">
                    <button type="submit" class="btn-confirm">
                        <Icon name="check" size=16 />
                        " Confirm"
                    </button>
                </div>
            </form>
        </Modal>
    }
}
