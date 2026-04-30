use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use leptos::{leptos_dom::helpers::set_timeout, prelude::*, reactive::owner::Owner};
use leptos_fluent::{I18n, move_tr};

use crate::components::icon::Icon;

/// A pending toast. Build via [`Toast::new`] and chaining helpers, then
/// call [`Toast::show`] to enqueue it.
///
/// The owner active when [`Toast::new`] is called is captured so that
/// [`Toast::show`] can resolve reactive contexts (e.g. [`ToastCtx`]) even
/// when it's called from a `spawn_local` or `set_timeout` callback where no
/// owner is active by default.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ToastKind {
    #[default]
    Info,
    Success,
    Error,
}

impl ToastKind {
    fn class(self) -> &'static str {
        match self {
            Self::Info => "toast--info",
            Self::Success => "toast--success",
            Self::Error => "toast--error",
        }
    }
}

pub struct Toast {
    message: Signal<String>,
    kind: ToastKind,
    action: Option<ToastAction>,
    auto_close: Option<Duration>,
    on_dismiss: Option<Callback<()>>,
    owner: Option<Owner>,
}

impl Toast {
    /// Sensible default for transient confirmation/error toasts: long
    /// enough to read a sentence, short enough to not linger.
    pub const DEFAULT_DURATION: Duration = Duration::from_secs(5);

    /// Create a toast with the default auto-close duration. Use
    /// [`Toast::persist`] to make it sticky, or [`Toast::auto_close`] to
    /// override the duration.
    pub fn new(message: impl Into<Signal<String>>) -> Self {
        Self {
            message: message.into(),
            kind: ToastKind::Info,
            action: None,
            auto_close: Some(Self::DEFAULT_DURATION),
            on_dismiss: None,
            owner: Owner::current(),
        }
    }

    /// Create an error toast (red accent + warning icon).
    pub fn error(message: impl Into<Signal<String>>) -> Self {
        Self::new(message).kind(ToastKind::Error)
    }

    /// Look up a translation reactively from the current i18n bundle.
    pub fn i18n(key: &'static str) -> Self {
        Self::new(Signal::derive(move || expect_context::<I18n>().tr(key)))
    }

    /// i18n + success kind.
    pub fn i18n_success(key: &'static str) -> Self {
        Self::i18n(key).kind(ToastKind::Success)
    }

    /// i18n + error kind.
    pub fn i18n_error(key: &'static str) -> Self {
        Self::i18n(key).kind(ToastKind::Error)
    }

    pub fn kind(mut self, kind: ToastKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_action(mut self, label: impl Into<Signal<String>>, on_click: Callback<()>) -> Self {
        self.action = Some(ToastAction {
            label: label.into(),
            on_click,
        });
        self
    }

    /// Keep the toast on screen until the user dismisses it manually.
    pub fn persist(mut self) -> Self {
        self.auto_close = None;
        self
    }

    pub fn on_dismiss(mut self, callback: Callback<()>) -> Self {
        self.on_dismiss = Some(callback);
        self
    }

    /// Enqueue the toast on the currently-provided [`ToastCtx`]. The owner
    /// captured at construction time is used, so this is safe to call from
    /// timers and async tasks.
    pub fn show(self) {
        let resolve_ctx = || use_context::<ToastCtx>();
        let ctx = match self.owner.as_ref() {
            Some(owner) => owner.with(resolve_ctx),
            None => resolve_ctx(),
        };
        let Some(ctx) = ctx else {
            return;
        };
        // Allocate entry signals under the context's owner (app root) so they
        // survive past the caller's scope — e.g. when an Effect that created
        // the toast re-runs before the user clicks dismiss.
        let (entry, auto_close) = ctx.owner.with(|| {
            let id = next_toast_id();
            (
                Entry {
                    id,
                    message: self.message,
                    kind: self.kind,
                    action: self.action,
                    on_dismiss: self.on_dismiss,
                    exiting: RwSignal::new(false),
                },
                self.auto_close,
            )
        });
        let id = entry.id;
        let signal = ctx.entries;
        signal.update(|toasts| toasts.push(entry));
        if let Some(duration) = auto_close {
            set_timeout(move || remove_toast(signal, id), duration);
        }
    }
}

#[derive(Clone)]
struct Entry {
    id: u64,
    message: Signal<String>,
    kind: ToastKind,
    action: Option<ToastAction>,
    on_dismiss: Option<Callback<()>>,
    exiting: RwSignal<bool>,
}

/// How long the fade-out animation lasts. Must match the CSS transition on
/// `.toast.exiting`.
const EXIT_ANIMATION: Duration = Duration::from_millis(250);

#[derive(Clone)]
struct ToastAction {
    label: Signal<String>,
    on_click: Callback<()>,
}

#[derive(Clone)]
pub struct ToastCtx {
    entries: RwSignal<Vec<Entry>>,
    // Owner of the scope where the context was provided (app root). Used to
    // allocate per-entry signals so they outlive the caller's scope — e.g. an
    // Effect that re-runs or a component that unmounts while the toast is
    // still on screen.
    owner: Owner,
}

static TOAST_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_toast_id() -> u64 {
    TOAST_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub fn provide_toast_context() {
    let owner = Owner::current().expect("provide_toast_context needs an active owner");
    provide_context(ToastCtx {
        entries: RwSignal::new(Vec::new()),
        owner,
    });
}

fn dismiss_toast(id: u64) {
    let Some(ctx) = use_context::<ToastCtx>() else {
        return;
    };
    remove_toast(ctx.entries, id);
}

fn remove_toast(signal: RwSignal<Vec<Entry>>, id: u64) {
    // Trigger the exit animation, then remove from the Vec once it's played
    // out. Calling remove_toast twice on the same entry is a no-op.
    let mut on_dismiss = None;
    let mut already_exiting = false;
    signal.with_untracked(|toasts| {
        if let Some(entry) = toasts.iter().find(|entry| entry.id == id) {
            if entry.exiting.get_untracked() {
                already_exiting = true;
            } else {
                entry.exiting.set(true);
                on_dismiss = entry.on_dismiss;
            }
        }
    });
    if already_exiting {
        return;
    }
    if let Some(callback) = on_dismiss {
        callback.run(());
    }
    set_timeout(
        move || {
            signal.update(|toasts| toasts.retain(|entry| entry.id != id));
        },
        EXIT_ANIMATION,
    );
}

#[component]
pub fn ToastContainer() -> impl IntoView {
    let entries = expect_context::<ToastCtx>().entries;
    view! {
        <div class="toast-container">
            <For
                each=move || entries.get()
                key=|entry| entry.id
                let:entry
            >
                <ToastView entry />
            </For>
        </div>
    }
}

#[component]
fn ToastView(entry: Entry) -> impl IntoView {
    let Entry {
        id,
        message,
        kind,
        action,
        exiting,
        ..
    } = entry;
    let dismiss_aria = move_tr!("toast-dismiss");
    let kind_class = kind.class();
    view! {
        <div class=format!("toast {kind_class}") class:exiting=move || exiting.get()>
            <span class="toast-icon" aria-hidden="true" />
            <span class="toast-message">{move || message.get()}</span>
            {action.map(|action| {
                let on_click = action.on_click;
                let label = action.label;
                view! {
                    <button
                        class="toast-action"
                        on:click=move |_| {
                            on_click.run(());
                            dismiss_toast(id);
                        }
                    >
                        {move || label.get()}
                    </button>
                }
            })}
            <button
                class="toast-dismiss"
                on:click=move |_| dismiss_toast(id)
                aria-label=move || dismiss_aria.get()
            >
                <Icon name="x" size=14 />
            </button>
        </div>
    }
}
