use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

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
/// `name` is the stable key. `label` and `description` are reactive so that
/// locale switches update text in place — children of `<For>` (which is keyed
/// by the stable `name`) subscribe via `move || opt.label.get()` instead of
/// being pinned to a value snapshot.
#[derive(Clone, Debug)]
pub struct DatalistOption {
    pub name: String,
    pub label: Signal<String>,
    pub description: Signal<String>,
    pub count: Option<u32>,
    /// When set, the option is shown but not selectable in the modal list,
    /// with the signal value as the reason.
    pub blocked_reason: Option<Signal<String>>,
}

// `name` is the stable identity (locale-stable). Reactive `label`/
// `description`/`blocked_reason` propagate updates through their own
// subscriptions, so equality on `name` + structure (count, blockedness)
// is enough for `Memo<Vec<DatalistOption>>` change detection — per-entry
// text changes are handled by the signal subscriptions inside the modal.
impl PartialEq for DatalistOption {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.count == other.count
            && self.blocked_reason.is_some() == other.blocked_reason.is_some()
    }
}

impl DatalistOption {
    /// Pass `Signal<String>`s that subscribe to whatever underlying source
    /// (locale resource, etc.) so the modal updates text in place.
    pub fn with_signals(
        name: impl Into<String>,
        label: Signal<String>,
        description: Signal<String>,
    ) -> Self {
        Self {
            name: name.into(),
            label,
            description,
            count: None,
            blocked_reason: None,
        }
    }

    pub fn with_count(mut self, count: u32) -> Self {
        self.count = Some(count);
        self
    }

    pub fn with_blocked_reason(mut self, reason: Signal<String>) -> Self {
        self.blocked_reason = Some(reason);
        self
    }
}

fn resolve_name(options: &[DatalistOption], input: &str) -> Option<String> {
    options
        .iter()
        .find(|opt| opt.name == input || opt.label.with_untracked(|s| s == input))
        .map(|opt| opt.name.clone())
}

fn render_badge(i18n: I18n, key: &'static str, count: u32) -> String {
    use std::collections::HashMap;
    let mut args = HashMap::new();
    args.insert("count".into(), count.into());
    i18n.tr_with_args(key, &args)
}

/// Shared state for the singleton `DatalistModal`. Each `DatalistInput`'s
/// Browse button calls `open()` with a snapshot of options + pick callback.
/// One modal instance lives in the DOM at a time — set up once at App root
/// via [`DatalistModal`].
#[derive(Clone, Copy)]
pub struct DatalistModalCtx {
    show: RwSignal<bool>,
    /// Snapshot of options at `open()` time. Captured as `Vec` (not `Signal`)
    /// because source signals live in per-iteration scopes that may be
    /// disposed while the modal is still open (e.g. when a label sync writes
    /// to the parent store and forces the iteration to re-render).
    options: RwSignal<Vec<DatalistOption>>,
    title: RwSignal<String>,
    badge_key: RwSignal<Option<&'static str>>,
    on_pick: StoredValue<Option<Arc<dyn Fn(String, String) + Send + Sync>>>,
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
        self.on_pick.set_value(Some(Arc::new(on_pick)));
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
    // `Arc<dyn Fn>` rather than `StoredValue` — the user callback often
    // mutates a parent store, whose write guard fires reactive notifications
    // that may dispose this very component's scope synchronously. A
    // `StoredValue` would be torn down mid-call and the next access would
    // panic; an `Arc` is owner-independent and lives until the last clone
    // (the event closure attached to the DOM node).
    let on_input: Arc<dyn Fn(String, Option<String>) + Send + Sync> = Arc::new(on_input);
    let on_input_change = Arc::clone(&on_input);
    let on_input_pick = Arc::clone(&on_input);

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
                    on_input_change(input, resolved);
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
                    let title = placeholder.get_untracked();
                    let opts = options.get();
                    let callback = Arc::clone(&on_input_pick);
                    ctx.open(opts, title, badge_key, move |label, name| {
                        display_value.set(label.clone());
                        callback(label, Some(name));
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
                    let label = opt.label;
                    let description = opt.description;
                    view! {
                        <option value=move || label.get()>
                            {move || description.with(|d| (!d.is_empty()).then(|| d.clone()))}
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
                    if query.is_empty() {
                        return true;
                    }
                    opt.name.to_lowercase().contains(&query)
                        || opt.label.with(|s| s.to_lowercase().contains(&query))
                        || opt.description.with(|s| s.to_lowercase().contains(&query))
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
                                        // `label` may change with locale; pick
                                        // the current value at click time.
                                        callback(label.get_untracked(), selected_name.clone());
                                    }
                                    ctx.show.set(false);
                                }
                            >
                                <div class="datalist-option-header">
                                    <span class="datalist-option-value">
                                        {move || label.get()}
                                    </span>
                                    {badge}
                                </div>
                                <div class="datalist-option-label">
                                    <Markdown text=description />
                                </div>
                                {blocked_reason.map(|reason| view! {
                                    <span class="datalist-option-blocked-reason">
                                        {move || reason.get()}
                                    </span>
                                })}
                            </button>
                        }
                    }
                />
            </div>
        </Modal>
    }
}
