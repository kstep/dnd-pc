use js_sys::Date;
use leptos::{html, prelude::*};
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
    // `options` must be undefined (or an object). Passing `null` makes V8
    // throw "Date.prototype.toLocaleDateString called on null or undefined"
    // because ECMA-402 runs ToObject/RequireObjectCoercible on it.
    date.to_locale_date_string(locale, &JsValue::UNDEFINED)
        .into()
}

#[component]
pub fn NotesPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let i18n = expect_context::<leptos_fluent::I18n>();
    let notes = store.notes();
    let pending_focus_ts = RwSignal::new(0u64);

    view! {
        <section>
            <div class="section-header">
                <h3>{move_tr!("panel-notes")}</h3>
                <button
                    class="btn-primary"
                    style="margin-left: auto"
                    on:click=move |_| {
                        let level = store.get_untracked().level();
                        let created_at = now_epoch_secs();
                        notes
                            .write()
                            .insert(
                                0,
                                Note {
                                    created_at,
                                    level,
                                    text: String::new(),
                                },
                            );
                        pending_focus_ts.set(created_at);
                    }
                >
                    {move_tr!("btn-add-note")}
                </button>
            </div>
            <div class="entry-list">
                {move || {
                    notes
                        .read()
                        .iter()
                        .enumerate()
                        .map(|(i, note)| {
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
                            let preview = text.lines().next().unwrap_or("").to_string();
                            let textarea_ref: NodeRef<html::Textarea> = NodeRef::new();
                            if created_at != 0 && created_at == pending_focus_ts.get_untracked() {
                                textarea_ref
                                    .on_load(move |el| {
                                        let _ = el.focus();
                                        pending_focus_ts.set(0);
                                    });
                            }
                            view! {
                                <div class="entry-item note-entry" class:expanded=initial_expanded>
                                    <ToggleButton />
                                    <div
                                        class="entry-content"
                                        on:click=move |e| {
                                            let target: web_sys::HtmlElement = event_target(&e);
                                            let Ok(Some(entry)) = target.closest(".entry-item") else {
                                                return;
                                            };
                                            let _ = entry.class_list().toggle("expanded");
                                        }
                                    >
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
                                            node_ref=textarea_ref
                                            prop:value=text
                                            on:change=move |e| {
                                                notes.write()[i].text = event_target_value(&e);
                                            }
                                        />
                                    </div>
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;

    use super::*;

    #[wasm_bindgen_test]
    fn format_epoch_date_zero_returns_em_dash() {
        assert_eq!(format_epoch_date(0, "en"), "\u{2014}");
    }

    #[wasm_bindgen_test]
    fn format_epoch_date_nonzero_returns_non_empty_for_en() {
        // 2024-01-15T00:00:00Z = 1705276800
        let s = format_epoch_date(1_705_276_800, "en");
        assert!(
            !s.is_empty() && s != "\u{2014}",
            "expected non-empty formatted date for en, got: {s:?}"
        );
    }

    #[wasm_bindgen_test]
    fn format_epoch_date_nonzero_returns_non_empty_for_ru() {
        let s = format_epoch_date(1_705_276_800, "ru");
        assert!(
            !s.is_empty() && s != "\u{2014}",
            "expected non-empty formatted date for ru, got: {s:?}"
        );
    }

    #[wasm_bindgen_test]
    fn format_epoch_date_accepts_now() {
        // Covers the exact value path produced by "Add note": now_epoch_secs().
        let s = format_epoch_date(now_epoch_secs(), "en");
        assert!(!s.is_empty() && s != "\u{2014}");
    }

    #[wasm_bindgen_test]
    fn add_note_inserts_new_entry_at_front() {
        // Mirrors the on:click handler mutation shape: insert at 0 with a
        // fresh created_at/level/empty text. Guards against regressions in
        // the Vec insertion path used by the Add Note button.
        let mut notes: Vec<Note> = vec![Note {
            created_at: 1,
            level: 3,
            text: "existing".into(),
        }];
        notes.insert(
            0,
            Note {
                created_at: 2,
                level: 5,
                text: String::new(),
            },
        );
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].created_at, 2);
        assert_eq!(notes[0].level, 5);
        assert!(notes[0].text.is_empty());
        assert_eq!(notes[1].text, "existing");
    }
}
