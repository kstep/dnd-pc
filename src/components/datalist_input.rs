use std::sync::atomic::{AtomicUsize, Ordering};

use leptos::prelude::*;

static DATALIST_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn next_datalist_id() -> String {
    let id = DATALIST_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("datalist-{id}")
}

fn resolve_name(options: &[(String, String, String)], input: &str) -> Option<String> {
    options
        .iter()
        .find(|(name, label, _)| label == input || name == input)
        .map(|(name, _, _)| name.clone())
}

/// A text input with an associated `<datalist>` for autocomplete suggestions.
///
/// Each instance generates a unique datalist ID internally.
/// A browse button (▾) opens a modal showing all options with descriptions,
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
    /// Autocomplete options as `(name, label, description)` triples.
    /// `name` is the stable key, `label` is the display text, `description` is
    /// shown below.
    #[prop(into)]
    options: Vec<(String, String, String)>,
    /// Called with `(input_text, resolved_name)` on each input event.
    /// `resolved_name` is `Some(name)` if the input matches an option's label
    /// or name.
    on_input: impl Fn(String, Option<String>) + Send + Sync + 'static,
) -> impl IntoView {
    let id = next_datalist_id();
    let datalist_id = id.clone();

    let show_modal = RwSignal::new(false);
    let search_query = RwSignal::new(String::new());
    let display_value = RwSignal::new(value);
    let options_stored = StoredValue::new(options);
    let on_input = StoredValue::new(on_input);

    let search_ref = NodeRef::<leptos::html::Input>::new();

    Effect::new(move || {
        if show_modal.get()
            && let Some(input) = search_ref.get()
        {
            let _ = input.focus();
        }
    });

    let filtered_options = move || {
        let query = search_query.get().to_lowercase();
        options_stored.with_value(|opts| {
            if query.is_empty() {
                return opts.clone();
            }
            opts.iter()
                .filter(|(name, label, description)| {
                    name.to_lowercase().contains(&query)
                        || label.to_lowercase().contains(&query)
                        || description.to_lowercase().contains(&query)
                })
                .cloned()
                .collect()
        })
    };

    let modal_title = placeholder.clone();

    view! {
        <div class="datalist-input-wrapper">
            <datalist id=datalist_id>
                {options_stored.with_value(|opts| {
                    opts.iter().map(|(_, label, description)| {
                        let label = label.clone();
                        let description = description.clone();
                        if description.is_empty() {
                            view! { <option value=label /> }.into_any()
                        } else {
                            view! { <option value=label>{description}</option> }.into_any()
                        }
                    }).collect_view()
                })}
            </datalist>
            <input
                type="text"
                class=class.unwrap_or_default()
                list=id
                placeholder=placeholder
                prop:value=move || display_value.get()
                on:change=move |event| {
                    let input = event_target_value(&event);
                    display_value.set(input.clone());
                    let resolved = options_stored.with_value(|opts| resolve_name(opts, &input));
                    on_input.with_value(|callback| callback(input, resolved));
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
                "▾"
            </button>
        </div>
        <Show when=move || show_modal.get()>
            <div
                class="datalist-modal-overlay"
                on:click=move |_| show_modal.set(false)
            >
                <div
                    class="datalist-modal"
                    on:click=move |event| event.stop_propagation()
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
                        node_ref=search_ref
                        type="search"
                        class="datalist-modal-search"
                        placeholder="Search…"
                        prop:value=move || search_query.get()
                        on:input=move |event| search_query.set(event_target_value(&event))
                    />
                    <div class="datalist-modal-list">
                        <For
                            each=filtered_options
                            key=|(name, _, _)| name.clone()
                            children=move |(name, label, description)| {
                                let selected_label = label.clone();
                                let selected_name = name.clone();
                                view! {
                                    <button
                                        type="button"
                                        class="datalist-option"
                                        on:click=move |_| {
                                            display_value.set(selected_label.clone());
                                            on_input.with_value(|callback| callback(selected_label.clone(), Some(selected_name.clone())));
                                            show_modal.set(false);
                                        }
                                    >
                                        <span class="datalist-option-value">{label}</span>
                                        <span class="datalist-option-label">{description}</span>
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
