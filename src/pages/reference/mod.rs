pub mod background;
pub mod class;
pub mod race;
pub mod spell;

use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_router::components::A;

use crate::{BASE_URL, rules::SpellList};

pub struct InlineSpell {
    pub label: String,
    pub level: u32,
    pub min_level: u32,
    pub sticky: bool,
    pub description: String,
}

pub enum FeatureSpells {
    None,
    Link(String),
    Inline(Vec<InlineSpell>),
}

impl FeatureSpells {
    pub fn from_spell_list(list: Option<&SpellList>) -> Self {
        match list {
            Some(SpellList::Ref { from }) => {
                let list_name = from
                    .strip_prefix("spells/")
                    .and_then(|s| s.strip_suffix(".json"))
                    .unwrap_or(from);
                Self::Link(list_name.to_string())
            }
            Some(SpellList::Inline(spells)) if !spells.is_empty() => Self::Inline(
                spells
                    .iter()
                    .map(|s| InlineSpell {
                        label: s.label().to_string(),
                        level: s.level,
                        min_level: s.min_level,
                        sticky: s.sticky,
                        description: s.description.clone(),
                    })
                    .collect(),
            ),
            _ => Self::None,
        }
    }
}

#[component]
pub fn FeatureSpellsView(spells: FeatureSpells) -> impl IntoView {
    match spells {
        FeatureSpells::Link(list_name) => view! {
            <p class="feature-spell-link">
                <A href=format!("{BASE_URL}/r/spell/{list_name}")>
                    {move_tr!("ref-spell-list-link")}
                </A>
            </p>
        }
        .into_any(),
        FeatureSpells::Inline(spells) => view! {
            <div class="feature-spells-inline">
                {spells.into_iter().map(|spell| {
                    let level_text = if spell.level == 0 {
                        move_tr!("ref-cantrips-level")
                    } else {
                        move_tr!("ref-spell-level", {"level" => spell.level.to_string()})
                    };
                    let min_level = spell.min_level;
                    let sticky = spell.sticky;
                    view! {
                        <div class="feature-spell-entry">
                            <strong>{spell.label}</strong>
                            {" ("}{level_text}
                            {sticky.then(|| view! {
                                {", "}{move_tr!("ref-spell-always-ready")}
                            })}
                            {(min_level > 0).then(|| view! {
                                {", "}{move_tr!("ref-spell-min-level", {"level" => min_level.to_string()})}
                            })}
                            {")"}
                            {(!spell.description.is_empty()).then(|| view! {
                                <p>{spell.description}</p>
                            })}
                        </div>
                    }
                }).collect_view()}
            </div>
        }
        .into_any(),
        FeatureSpells::None => ().into_any(),
    }
}
