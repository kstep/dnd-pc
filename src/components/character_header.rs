use leptos::prelude::*;
use strum::IntoEnumIterator;

use crate::model::{Alignment, Character};

#[component]
pub fn CharacterHeader() -> impl IntoView {
    let char_signal = expect_context::<RwSignal<Character>>();

    let name = Memo::new(move |_| char_signal.get().identity.name.clone());
    let class = Memo::new(move |_| char_signal.get().identity.class.clone());
    let level = Memo::new(move |_| char_signal.get().identity.level);
    let race = Memo::new(move |_| char_signal.get().identity.race.clone());
    let background = Memo::new(move |_| char_signal.get().identity.background.clone());
    let alignment = Memo::new(move |_| char_signal.get().identity.alignment);
    let xp = Memo::new(move |_| char_signal.get().identity.experience_points);
    let prof_bonus = Memo::new(move |_| char_signal.get().proficiency_bonus());

    view! {
        <div class="panel character-header">
            <div class="header-row">
                <div class="header-field name-field">
                    <label>"Character Name"</label>
                    <input
                        type="text"
                        prop:value=name
                        on:input=move |e| {
                            char_signal.update(|c| c.identity.name = event_target_value(&e));
                        }
                    />
                </div>
                <div class="header-field">
                    <label>"Class"</label>
                    <input
                        type="text"
                        prop:value=class
                        on:input=move |e| {
                            char_signal.update(|c| c.identity.class = event_target_value(&e));
                        }
                    />
                </div>
                <div class="header-field level-field">
                    <label>"Level"</label>
                    <input
                        type="number"
                        min="1"
                        max="20"
                        prop:value=move || level.get().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                char_signal.update(|c| c.identity.level = v.clamp(1, 20));
                            }
                        }
                    />
                </div>
                <div class="header-field">
                    <label>"Race"</label>
                    <input
                        type="text"
                        prop:value=race
                        on:input=move |e| {
                            char_signal.update(|c| c.identity.race = event_target_value(&e));
                        }
                    />
                </div>
                <div class="header-field">
                    <label>"Background"</label>
                    <input
                        type="text"
                        prop:value=background
                        on:input=move |e| {
                            char_signal.update(|c| c.identity.background = event_target_value(&e));
                        }
                    />
                </div>
                <div class="header-field">
                    <label>"Alignment"</label>
                    <select
                        on:change=move |e| {
                            let val = event_target_value(&e);
                            if let Ok(a) = serde_json::from_str::<Alignment>(&format!("\"{val}\"")) {
                                char_signal.update(|c| c.identity.alignment = a);
                            }
                        }
                    >
                        {Alignment::iter()
                            .map(|a| {
                                let label = a.to_string();
                                let val = format!("{a:?}");
                                let selected = move || alignment.get() == a;
                                view! {
                                    <option value=val.clone() selected=selected>
                                        {label}
                                    </option>
                                }
                            })
                            .collect_view()}
                    </select>
                </div>
                <div class="header-field level-field">
                    <label>"XP"</label>
                    <input
                        type="number"
                        min="0"
                        prop:value=move || xp.get().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                char_signal.update(|c| c.identity.experience_points = v);
                            }
                        }
                    />
                </div>
                <div class="header-field level-field">
                    <label>"Prof. Bonus"</label>
                    <span class="computed-value">"+" {prof_bonus}</span>
                </div>
            </div>
            <a href="/" class="back-link">"< Back to Characters"</a>
        </div>
    }
}
