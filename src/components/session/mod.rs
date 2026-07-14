mod backpack;
mod choices;
mod damage_modifiers;
mod effects;
mod gear_actions;
mod languages;
mod resources;
mod senses;
mod spells;
mod stats;
mod weapons;

pub use backpack::BackpackBlock;
pub use choices::ChoicesBlock;
pub use damage_modifiers::DamageModifiersBlock;
pub use effects::EffectsBlock;
pub use gear_actions::GearActionsBlock;
pub use languages::LanguagesBlock;
use leptos::prelude::*;
pub use resources::ResourcesBlock;
pub use senses::SensesBlock;
pub use spells::SpellsBlock;
pub use stats::{StatsBlock, adv_icon};
pub use weapons::WeaponsBlock;

#[component]
pub fn FreeUsesBadge(available: u32, max: u32) -> impl IntoView {
    view! { <span class="entry-badge">{available} "/" {max}</span> }
}
