use std::sync::atomic::{AtomicUsize, Ordering};

use leptos::prelude::*;
use leptos_fluent::{I18n, move_tr};
use leptos_router::components::A;

use crate::components::{icon::Icon, modal::Modal};

static DATALIST_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn next_datalist_id() -> usize {
    DATALIST_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// An entry shown in the `DatalistInput` suggestions list.
///
/// `name` is the stable key, `label` is the display text, `description` is the
/// secondary line in the modal. `count` is an optional numeric meta used to
/// render a localized badge via the `badge_key` Fluent message (see
/// `DatalistInput::badge_key`).
#[derive(Clone, Debug, PartialEq)]
pub struct DatalistOption {
    pub name: String,
    pub label: String,
    pub description: String,
    pub count: Option<u32>,
    /// When set, the option is shown but not selectable in the modal list,
    /// with this string as the reason.
    pub blocked_reason: Option<String>,
}

impl DatalistOption {
    pub fn new(
        name: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            description: description.into(),
            count: None,
            blocked_reason: None,
        }
    }

    pub fn with_count(mut self, count: u32) -> Self {
        self.count = Some(count);
        self
    }

    pub fn with_blocked_reason(mut self, reason: impl Into<String>) -> Self {
        self.blocked_reason = Some(reason.into());
        self
    }
}

fn resolve_name(options: &[DatalistOption], input: &str) -> Option<String> {
    options
        .iter()
        .find(|opt| opt.label == input || opt.name == input)
        .map(|opt| opt.name.clone())
}

fn render_badge(i18n: I18n, key: &'static str, count: u32) -> String {
    use std::collections::HashMap;
    let mut args = HashMap::new();
    args.insert("count".into(), count.into());
    i18n.tr_with_args(key, &args)
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
    value: Signal<String>,
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
    /// Autocomplete options. `name` is the stable key, `label` is the display
    /// text, `description` is shown below. Set `count` on an option to render
    /// a badge (requires `badge_key`).
    #[prop(into)]
    options: Signal<Vec<DatalistOption>>,
    /// Fluent message id used to render a badge in the modal list for each
    /// option that has a `count`. The message is called with `$count = n`.
    #[prop(optional)]
    badge_key: Option<&'static str>,
    /// Whether the input is required for form validation.
    #[prop(optional)]
    required: bool,
    /// Called with `(input_text, resolved_name)` on each input event.
    /// `resolved_name` is `Some(name)` if the input matches an option's label
    /// or name.
    on_input: impl Fn(String, Option<String>) + Send + Sync + 'static,
) -> impl IntoView {
    let id = next_datalist_id();

    let show_modal = RwSignal::new(false);
    let search_query = RwSignal::new(String::new());
    let display_value = RwSignal::new(value.get_untracked());

    // Sync display_value when external value changes (e.g. parent resets it)
    Effect::new(move || {
        display_value.set(value.get());
    });
    let on_input = StoredValue::new(on_input);
    let focused = RwSignal::new(false);

    let filtered_options = move || {
        let query = search_query.get().to_lowercase();
        options.with(|opts| {
            opts.iter()
                .filter(|opt| {
                    query.is_empty()
                        || opt.name.to_lowercase().contains(&query)
                        || opt.label.to_lowercase().contains(&query)
                        || opt.description.to_lowercase().contains(&query)
                })
                .cloned()
                .collect::<Vec<_>>()
        })
    };

    let i18n = expect_context::<I18n>();

    view! {
        <div class=format!("datalist-input-wrapper {}", class.unwrap_or_default())>
            <datalist id=format!("datalist-{id}")>
                {move || focused.get().then(|| options.with(|opts| {
                    opts.iter().map(|opt| {
                        let label = opt.label.clone();
                        let description = opt.description.clone();
                        view! {
                            <option value=label>
                                {(!description.is_empty()).then_some(description)}
                            </option>
                        }
                    }).collect_view()
                }))}
            </datalist>
            // NB: on:change reads from `options` signal directly (not the DOM
            // datalist), so it works even after the datalist options are cleared.
            <input
                type="text"
                required=required
                list=format!("datalist-{id}")
                placeholder=move || placeholder.get()
                prop:value=move || display_value.get()
                on:focus=move |_| focused.set(true)
                on:blur=move |_| focused.set(false)
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
                <Icon name="chevron-down" />
            </button>
        </div>
        <Modal show=show_modal title=placeholder>
            <input
                autofocus
                type="search"
                class="datalist-modal-search"
                placeholder=move || move_tr!("search").get()
                prop:value=move || search_query.get()
                on:input=move |event| search_query.set(event_target_value(&event))
            />
            <div class="datalist-modal-list">
                <For
                    each=filtered_options
                    key=|opt| opt.name.clone()
                    children=move |opt| {
                        let DatalistOption {
                            name,
                            label,
                            description,
                            count,
                            blocked_reason,
                        } = opt;
                        let selected_label = label.clone();
                        let selected_name = name.clone();
                        let badge = badge_key.zip(count).map(|(key, n)| view! {
                            <span class="datalist-option-badge">
                                {move || render_badge(i18n, key, n)}
                            </span>
                        });
                        let is_blocked = blocked_reason.is_some();
                        view! {
                            <button
                                type="button"
                                class="datalist-option"
                                class:datalist-option-blocked=is_blocked
                                disabled=is_blocked
                                on:click=move |_| {
                                    let label = selected_label.clone();
                                    display_value.set(label.clone());
                                    on_input.with_value(|callback| callback(label, Some(selected_name.clone())));
                                    show_modal.set(false);
                                }
                            >
                                <div class="datalist-option-header">
                                    <span class="datalist-option-value">{label}</span>
                                    {badge}
                                </div>
                                <span class="datalist-option-label">{description}</span>
                                {blocked_reason.map(|reason| view! {
                                    <span class="datalist-option-blocked-reason">{reason}</span>
                                })}
                            </button>
                        }
                    }
                />
            </div>
        </Modal>
    }
}
