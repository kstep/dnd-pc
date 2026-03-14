use std::sync::atomic::{AtomicUsize, Ordering};

use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_router::components::A;

use crate::components::icon::Icon;

static DATALIST_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn next_datalist_id() -> usize {
    DATALIST_COUNTER.fetch_add(1, Ordering::Relaxed)
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
    placeholder: Signal<String>,
    /// CSS class for the input
    #[prop(into, optional)]
    class: Option<String>,
    /// Optional href for reference link icon shown between input and browse
    /// button. When `None` (default), the icon is hidden.
    #[prop(into, optional)]
    ref_href: Signal<Option<String>>,
    /// Autocomplete options as `(name, label, description)` triples.
    /// `name` is the stable key, `label` is the display text, `description` is
    /// shown below.
    #[prop(into)]
    options: Signal<Vec<(String, String, String)>>,
    /// Called with `(input_text, resolved_name)` on each input event.
    /// `resolved_name` is `Some(name)` if the input matches an option's label
    /// or name.
    on_input: impl Fn(String, Option<String>) + Send + Sync + 'static,
) -> impl IntoView {
    let id = next_datalist_id();

    let show_modal = RwSignal::new(false);
    let search_query = RwSignal::new(String::new());
    let display_value = RwSignal::new(value);
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
        options.with(|opts| {
            opts.iter()
                .filter(|(name, label, description)| {
                    query.is_empty()
                        || name.to_lowercase().contains(&query)
                        || label.to_lowercase().contains(&query)
                        || description.to_lowercase().contains(&query)
                })
                .cloned()
                .collect::<Vec<_>>()
        })
    };

    view! {
        <div class=format!("datalist-input-wrapper {}", class.unwrap_or_default())>
            <datalist id=format!("datalist-{id}")>
                {move || options.with(|opts| {
                    opts.iter().map(|(_, label, description)| {
                        let label = label.clone();
                        let description = description.clone();
                        view! {
                            <option value=label>
                                {(!description.is_empty()).then_some(description)}
                            </option>
                        }
                    }).collect_view()
                })}
            </datalist>
            <input
                type="text"
                list=format!("datalist-{id}")
                placeholder=move || placeholder.get()
                prop:value=move || display_value.get()
                on:change=move |event| {
                    let input = event_target_value(&event);
                    display_value.set(input.clone());
                    let resolved = options.with(|opts| resolve_name(opts, &input));
                    on_input.with_value(|callback| callback(input, resolved));
                }
            />
            {move || ref_href.get().map(|href| view! {
                <A href=href attr:class="datalist-ref-link" attr:title="Reference">
                    <Icon name="info" size=12 />
                </A>
            })}
            <button
                type="button"
                class="datalist-browse-btn"
                title=move_tr!("browse-options")
                on:click=move |_| {
                    search_query.set(String::new());
                    show_modal.set(true);
                }
            >
                <Icon name="chevron-down" size=14 />
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
                        <span>{move || placeholder.get()}</span>
                        <button
                            type="button"
                            class="datalist-modal-close"
                            on:click=move |_| show_modal.set(false)
                        >
                            <Icon name="x" size=20 />
                        </button>
                    </div>
                    <input
                        node_ref=search_ref
                        type="search"
                        class="datalist-modal-search"
                        placeholder=move || move_tr!("search").get()
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
                                            let label = selected_label.clone();
                                            display_value.set(label.clone());
                                            on_input.with_value(|callback| callback(label, Some(selected_name.clone())));
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
