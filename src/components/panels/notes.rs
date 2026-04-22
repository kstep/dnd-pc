use js_sys::Date;
use leptos::prelude::*;
use leptos_fluent::{move_tr, tr};
use reactive_stores::Store;
use wasm_bindgen::JsValue;

use crate::{
    components::{icon::Icon, toggle_button::ToggleButton},
    model::{Character, CharacterStoreFields, Note, now_epoch_secs},
};

/// Render an epoch-seconds timestamp in the current UI locale, or `—` when
/// unknown.
fn format_epoch_date(ts: u64, locale: &str) -> String {
    if ts == 0 {
        return "\u{2014}".into();
    }
    let date = Date::new(&((ts as f64) * 1000.0).into());
    date.to_locale_date_string(locale, &JsValue::NULL).into()
}

#[component]
pub fn NotesPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let i18n = expect_context::<leptos_fluent::I18n>();
    let notes = store.notes();

    view! {
        <section>
            <div class="section-header">
                <h3>{move_tr!("panel-notes")}</h3>
            </div>
            <button
                class="btn-primary"
                on:click=move |_| {
                    let level = store.get_untracked().level();
                    notes.write().insert(0, Note {
                        created_at: now_epoch_secs(),
                        level,
                        text: String::new(),
                    });
                }
            >
                {move_tr!("btn-add-note")}
            </button>
            <div class="entry-list">
                {move || notes.read().iter().enumerate().map(|(i, note)| {
                    let text = note.text.clone();
                    let level = note.level;
                    let created_at = note.created_at;
                    let initial_expanded = text.is_empty();
                    let level_label = if level > 0 {
                        tr!("slot-level", {"level" => level})
                    } else {
                        "\u{2014}".into()
                    };
                    let date_label = format_epoch_date(created_at, i18n.language.get().id);
                    let preview = text
                        .lines()
                        .next()
                        .unwrap_or("")
                        .to_string();
                    view! {
                        <div class="entry-item" class:expanded=initial_expanded>
                            <ToggleButton />
                            <div class="entry-content">
                                <span class="note-level">{level_label}</span>
                                <span class="note-date">{date_label}</span>
                                <span class="note-preview">{preview}</span>
                            </div>
                            <div class="entry-actions">
                                <button
                                    class="btn-remove"
                                    on:click=move |_| {
                                        if i < notes.read().len() {
                                            notes.write().remove(i);
                                        }
                                    }
                                >
                                    <Icon name="x" />
                                </button>
                            </div>
                            <div class="entry-full-row">
                                <textarea
                                    class="notes-textarea"
                                    prop:value=text
                                    on:change=move |e| {
                                        notes.write()[i].text = event_target_value(&e);
                                    }
                                />
                            </div>
                        </div>
                    }
                }).collect_view()}
            </div>
        </section>
    }
}
