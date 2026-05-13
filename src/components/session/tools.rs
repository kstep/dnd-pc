use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::stat_box::StatBox,
    model::{Character, ProficiencyLevel, format_bonus},
};

#[component]
pub fn ToolsBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    move || {
        let rows: Vec<(String, bool, i32)> = store
            .read()
            .tools()
            .filter(|(entry, _)| !entry.name.is_empty())
            .map(|(entry, bonus)| {
                (
                    entry.name.clone(),
                    entry.prof != ProficiencyLevel::None,
                    bonus,
                )
            })
            .collect();

        (!rows.is_empty()).then(|| {
            view! {
                <h4 class="session-subsection-title">{move_tr!("session-tools")}</h4>
                <div class="slot-box-list">
                    {rows.into_iter().map(|(name, proficient, bonus)| {
                        let label = Signal::derive(move || name.clone());
                        let highlighted = Signal::derive(move || proficient);
                        view! {
                            <StatBox label=label highlighted=highlighted>
                                <span class="stat-highlight">{format_bonus(bonus)}</span>
                            </StatBox>
                        }
                    }).collect_view()}
                </div>
            }
        })
    }
}
