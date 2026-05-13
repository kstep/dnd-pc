use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::stat_box::StatBox,
    model::{Character, format_bonus},
};

#[component]
pub fn ToolsBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    move || {
        let rows: Vec<(String, i32)> = store
            .read()
            .tools()
            .map(|(entry, bonus)| (entry.name.clone(), bonus))
            .collect();

        (!rows.is_empty()).then(|| {
            view! {
                <h4 class="session-subsection-title">{move_tr!("session-tools")}</h4>
                <div class="slot-box-list">
                    {rows.into_iter().map(|(name, bonus)| {
                        let label = Signal::derive(move || name.clone());
                        view! {
                            <StatBox label=label highlighted=Signal::derive(|| true)>
                                <span class="stat-highlight">{format_bonus(bonus)}</span>
                            </StatBox>
                        }
                    }).collect_view()}
                </div>
            }
        })
    }
}
