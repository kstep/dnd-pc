use std::sync::atomic::{AtomicUsize, Ordering};

use leptos::prelude::*;

static DATALIST_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn next_datalist_id() -> String {
    let id = DATALIST_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("datalist-{id}")
}

/// A text input with an associated `<datalist>` for autocomplete suggestions.
///
/// Each instance generates a unique datalist ID internally.
/// A browse button (📋) opens a modal showing all options with descriptions,
/// providing consistent behaviour across mobile platforms.
#[component]
pub fn DatalistInput(
    /// Current input value
    #[prop(into)]
    value: String,
    /// Placeholder text
    #[prop(into)]
    placeholder: String,
    /// CSS class for the input
    #[prop(into, optional)]
    class: Option<String>,
    /// Autocomplete options as `(value, label)` pairs.
    /// If label is empty, only the value is shown.
    #[prop(into)]
    options: Vec<(String, String)>,
    /// Called with the new value on each input event
    on_input: impl Fn(String) + Send + Sync + 'static,
) -> impl IntoView {
    let datalist_id = next_datalist_id();
    let list_id = datalist_id.clone();

    let show_modal = RwSignal::new(false);
    let search_query = RwSignal::new(String::new());
    let options_stored = StoredValue::new(options);
    let on_input_stored = StoredValue::new(on_input);

    let filtered_options = move || {
        let query = search_query.get().to_lowercase();
        options_stored.with_value(|opts| {
            opts.iter()
                .filter(|(val, label)| {
                    query.is_empty()
                        || val.to_lowercase().contains(&query)
                        || label.to_lowercase().contains(&query)
                })
                .cloned()
                .collect::<Vec<_>>()
        })
    };

    let modal_title = placeholder.clone();

    view! {
        <div class="datalist-input-wrapper">
            <datalist id=datalist_id>
                {options_stored.with_value(|opts| {
                    opts.iter().map(|(val, label)| {
                        let val = val.clone();
                        let label = label.clone();
                        if label.is_empty() {
                            view! { <option value=val /> }.into_any()
                        } else {
                            view! { <option value=val>{label}</option> }.into_any()
                        }
                    }).collect_view()
                })}
            </datalist>
            <input
                type="text"
                class=class.unwrap_or_default()
                list=list_id
                placeholder=placeholder
                prop:value=value
                on:input=move |e| {
                    on_input_stored.with_value(|f| f(event_target_value(&e)));
                }
            />
            <button
                type="button"
                class="datalist-browse-btn"
                title="Browse options"
                on:click=move |_| {
                    search_query.set(String::new());
                    show_modal.set(true);
                }
            >
                "📋"
            </button>
        </div>
        <Show when=move || show_modal.get()>
            <div
                class="datalist-modal-overlay"
                on:click=move |_| show_modal.set(false)
            >
                <div
                    class="datalist-modal"
                    on:click=move |e| e.stop_propagation()
                >
                    <div class="datalist-modal-header">
                        <span>{modal_title.clone()}</span>
                        <button
                            type="button"
                            class="datalist-modal-close"
                            on:click=move |_| show_modal.set(false)
                        >
                            "✕"
                        </button>
                    </div>
                    <input
                        type="search"
                        class="datalist-modal-search"
                        placeholder="Search…"
                        prop:value=move || search_query.get()
                        on:input=move |e| search_query.set(event_target_value(&e))
                    />
                    <div class="datalist-modal-list">
                        <For
                            each=filtered_options
                            key=|(val, _)| val.clone()
                            children=move |(val, label)| {
                                let selected_value = val.clone();
                                view! {
                                    <button
                                        type="button"
                                        class="datalist-option"
                                        on:click=move |_| {
                                            on_input_stored.with_value(|f| f(selected_value.clone()));
                                            show_modal.set(false);
                                        }
                                    >
                                        <span class="datalist-option-value">{val.clone()}</span>
                                        {(!label.is_empty()).then(|| view! {
                                            <span class="datalist-option-label">{label.clone()}</span>
                                        })}
                                    </button>
                                }
                            }
                        />
                    </div>
                </div>
            </div>
        </Show>
    }
}
