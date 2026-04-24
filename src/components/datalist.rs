use std::sync::atomic::{AtomicUsize, Ordering};

use leptos::prelude::*;
use leptos_fluent::{I18n, move_tr};

use crate::components::{icon::Icon, markdown::Markdown, modal::Modal, ref_link::Ref};

static DATALIST_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Generate a unique `<datalist>` id for per-instance use. Call-sites that
/// need one native datalist per input (no cross-input sharing) can use this
/// and pair it with a `<SharedDatalist>` helper.
pub fn next_datalist_id() -> String {
    let n = DATALIST_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("datalist-{n}")
}

/// An entry shown in the `DatalistInput` suggestions list and the shared
/// `DatalistModal` browse view.
///
/// `name` is the stable key, `label` is the display text, `description` is the
/// secondary line in the modal. `count` is an optional numeric meta used to
/// render a localized badge via the `badge_key` Fluent message.
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

/// Shared state for the singleton `DatalistModal`. Each `DatalistInput`'s
/// Browse button calls `open()` with its options + pick callback; the modal
/// displays them. Only one modal instance lives in the DOM at a time — set
/// up once at App root via [`DatalistModal`].
#[derive(Clone, Copy)]
pub struct DatalistModalCtx {
    show: RwSignal<bool>,
    options: RwSignal<Vec<DatalistOption>>,
    title: RwSignal<String>,
    badge_key: RwSignal<Option<&'static str>>,
    on_pick: StoredValue<Option<Callback<(String, String), ()>>>,
}

impl DatalistModalCtx {
    pub fn new() -> Self {
        Self {
            show: RwSignal::new(false),
            options: RwSignal::new(Vec::new()),
            title: RwSignal::new(String::new()),
            badge_key: RwSignal::new(None),
            on_pick: StoredValue::new(None),
        }
    }

    pub fn open(
        &self,
        options: Vec<DatalistOption>,
        title: String,
        badge_key: Option<&'static str>,
        on_pick: impl Fn(String, String) + Send + Sync + 'static,
    ) {
        self.options.set(options);
        self.title.set(title);
        self.badge_key.set(badge_key);
        self.on_pick
            .set_value(Some(Callback::new(move |(label, name)| {
                on_pick(label, name)
            })));
        self.show.set(true);
    }
}

impl Default for DatalistModalCtx {
    fn default() -> Self {
        Self::new()
    }
}

/// Install a fresh `DatalistModalCtx` in the current reactive scope.
pub fn provide_datalist_modal_ctx() -> DatalistModalCtx {
    let ctx = DatalistModalCtx::new();
    provide_context(ctx);
    ctx
}

/// A text input with an associated `<datalist>` for autocomplete and a browse
/// button that opens the shared `DatalistModal`. The `<datalist>` element
/// itself is **owned by the parent** and referenced via `list_id`; this allows
/// N inputs to share one data list. The browse-modal is a singleton mounted
/// at App root — see [`DatalistModal`].
#[component]
pub fn DatalistInput(
    /// Current input value
    #[prop(into)]
    value: Signal<String>,
    /// Placeholder text + title used as modal title when Browse is clicked.
    #[prop(into)]
    placeholder: Signal<String>,
    /// CSS class for the input
    #[prop(into, optional)]
    class: Option<String>,
    /// Optional href for reference link icon shown between input and browse
    /// button. When `None` (default), the icon is hidden.
    #[prop(into, optional)]
    ref_href: Signal<Option<String>>,
    /// The id of a `<datalist>` element mounted elsewhere in the DOM. Inputs
    /// with matching `list_id` share the same native autocomplete list.
    #[prop(into)]
    list_id: Signal<String>,
    /// Autocomplete options. Used for (1) resolving typed values against
    /// option labels in `on:change`, (2) feeding the Browse modal when opened.
    /// Not rendered inline by this component — the parent owns the native
    /// `<datalist>`.
    #[prop(into)]
    options: Signal<Vec<DatalistOption>>,
    /// Fluent message id used to render a badge in the modal list for each
    /// option that has a `count`. The message is called with `$count = n`.
    #[prop(optional)]
    badge_key: Option<&'static str>,
    /// Whether the input is required for form validation.
    #[prop(optional)]
    required: bool,
    /// Called with `(input_text, resolved_name)` on each change event.
    /// `resolved_name` is `Some(name)` if the input matches an option's label
    /// or name (or was picked from the modal).
    on_input: impl Fn(String, Option<String>) + Send + Sync + 'static,
) -> impl IntoView {
    let ctx = expect_context::<DatalistModalCtx>();
    let display_value = RwSignal::new(value.get_untracked());
    // Sync display_value when external value changes (e.g. parent resets it)
    Effect::new(move || {
        display_value.set(value.get());
    });
    let on_input = StoredValue::new(on_input);

    view! {
        <div class=format!("datalist-input-wrapper {}", class.unwrap_or_default())>
            <input
                type="text"
                required=required
                list=move || list_id.get()
                placeholder=move || placeholder.get()
                prop:value=move || display_value.get()
                on:change=move |event| {
                    let input = event_target_value(&event);
                    display_value.set(input.clone());
                    let resolved = options.with_untracked(|opts| resolve_name(opts, &input));
                    on_input.with_value(|callback| callback(input, resolved));
                }
            />
            {move || ref_href.get().map(|href| view! {
                <Ref href=href attr:class="datalist-ref-link" attr:title="Reference">
                    <Icon name="info" size=12 />
                </Ref>
            })}
            <button
                type="button"
                class="datalist-browse-btn"
                title=move_tr!("browse-options")
                on:click=move |_| {
                    let opts = options.get_untracked();
                    let title = placeholder.get_untracked();
                    ctx.open(opts, title, badge_key, move |label, name| {
                        display_value.set(label.clone());
                        on_input.with_value(|callback| callback(label, Some(name)));
                    });
                }
            >
                <Icon name="chevron-down" />
            </button>
        </div>
    }
}

/// Native `<datalist>` with `<option>` children, mountable once to be shared
/// by N `DatalistInput`s via matching `list_id`. Option elements re-render
/// reactively when `options` changes.
#[component]
pub fn SharedDatalist(
    #[prop(into)] id: Signal<String>,
    #[prop(into)] options: Signal<Vec<DatalistOption>>,
) -> impl IntoView {
    view! {
        <datalist id=move || id.get()>
            {move || options.with(|opts| {
                opts.iter().map(|opt| {
                    let label = opt.label.clone();
                    let description = opt.description.clone();
                    view! {
                        <option value=label>
                            {(!description.is_empty()).then_some(description)}
                        </option>
                    }
                }).collect_view()
            })}
        </datalist>
    }
}

/// Singleton modal that serves as the "browse options" view for every
/// `DatalistInput` in the application. Mount once at App root **after**
/// calling [`provide_datalist_modal_ctx`]. Reads `options`/`title`/`badge_key`
/// from the ctx; calls the stored `on_pick` when the user selects an option.
#[component]
pub fn DatalistModal() -> impl IntoView {
    let ctx = expect_context::<DatalistModalCtx>();
    let i18n = expect_context::<I18n>();
    let search_query = RwSignal::new(String::new());

    // Reset search when opening (otherwise it retains the previous query).
    Effect::new(move || {
        if ctx.show.get() {
            search_query.set(String::new());
        }
    });

    let title_signal = Signal::derive(move || ctx.title.get());

    let filtered = move || {
        let query = search_query.get().to_lowercase();
        ctx.options.with(|opts| {
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

    view! {
        <Modal show=ctx.show title=title_signal>
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
                    each=filtered
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
                        let badge = ctx.badge_key.get_untracked().zip(count).map(|(key, n)| {
                            view! {
                                <span class="datalist-option-badge">
                                    {move || render_badge(i18n, key, n)}
                                </span>
                            }
                        });
                        let is_blocked = blocked_reason.is_some();
                        view! {
                            <button
                                type="button"
                                class="datalist-option"
                                class:datalist-option-blocked=is_blocked
                                disabled=is_blocked
                                on:click=move |_| {
                                    if let Some(callback) = ctx.on_pick.get_value() {
                                        callback.run((selected_label.clone(), selected_name.clone()));
                                    }
                                    ctx.show.set(false);
                                }
                            >
                                <div class="datalist-option-header">
                                    <span class="datalist-option-value">{label}</span>
                                    {badge}
                                </div>
                                <div class="datalist-option-label">
                                    <Markdown text=description />
                                </div>
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
