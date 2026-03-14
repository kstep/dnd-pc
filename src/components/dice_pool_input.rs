use std::collections::BTreeMap;

use leptos::{html, prelude::*};

use crate::{components::icon::Icon, expr::DicePool};

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

    let groups_confirm = groups.clone();

    let confirm = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        let mut pool: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for (&sides, refs) in &groups_confirm {
            let values: Vec<u32> = refs
                .iter()
                .map(|input_ref| {
                    input_ref
                        .get()
                        .and_then(|el| el.value().parse::<u32>().ok())
                        .unwrap_or(1)
                })
                .collect();
            pool.insert(sides, values);
        }
        on_confirm.with_value(|f| f(pool.into()));
        show.set(false);
    };

    // Build group views eagerly to avoid ownership issues
    let group_views = groups
        .into_iter()
        .map(|(sides, refs): (u32, Vec<NodeRef<html::Input>>)| {
            let input_views = refs
                .into_iter()
                .map(|node_ref| {
                    view! {
                        <input
                            type="number"
                            min=1
                            max=sides
                            value=1
                            required
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
        .collect_view();

    view! {
        <div class="datalist-modal-overlay" class:hidden=move || !show.get() on:click=move |_| show.set(false)>
            <form
                class="datalist-modal dice-pool-form"
                on:click=move |event: web_sys::MouseEvent| {
                    event.stop_propagation();
                }
                on:submit=confirm
            >
                <div class="datalist-modal-header">
                    <span>"Dice Rolls"</span>
                    <button
                        type="button"
                        class="datalist-modal-close"
                        on:click=move |_| show.set(false)
                    >
                        <Icon name="x" size=20 />
                    </button>
                </div>
                <div class="dice-pool-groups">{group_views}</div>
                <div class="dice-pool-footer">
                    <button type="submit" class="btn-confirm">
                        <Icon name="check" size=16 />
                        " Confirm"
                    </button>
                </div>
            </form>
        </div>
    }
}
