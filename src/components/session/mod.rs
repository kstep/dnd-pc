mod backpack;
mod choices;
mod damage_modifiers;
mod effects;
mod languages;
mod resources;
mod spells;
mod stats;
mod weapons;

pub use backpack::BackpackBlock;
pub use choices::ChoicesBlock;
pub use damage_modifiers::DamageModifiersBlock;
pub use effects::EffectsBlock;
pub use languages::LanguagesBlock;
use leptos::prelude::*;
pub use resources::ResourcesBlock;
pub use spells::SpellsBlock;
pub use stats::{StatsBlock, adv_icon};
pub use weapons::WeaponsBlock;

#[component]
pub fn FreeUsesBadge(available: u32, max: u32) -> impl IntoView {
    view! {
        <span class="entry-badge">
            {available} "/" {max}
        </span>
    }
}
