use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use leptos::{leptos_dom::helpers::set_timeout, prelude::*, reactive::owner::Owner};
use leptos_fluent::move_tr;

use crate::components::icon::Icon;

/// A pending toast. Build via [`Toast::new`] and chaining helpers, then
/// call [`Toast::show`] to enqueue it.
///
/// The owner active when [`Toast::new`] is called is captured so that
/// [`Toast::show`] can resolve reactive contexts (e.g. [`ToastCtx`]) even
/// when it's called from a `spawn_local` or `set_timeout` callback where no
/// owner is active by default.
pub struct Toast {
    message: String,
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
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            action: None,
            auto_close: Some(Self::DEFAULT_DURATION),
            on_dismiss: None,
            owner: Owner::current(),
        }
    }

    pub fn with_action(mut self, label: impl Into<String>, on_click: Callback<()>) -> Self {
        self.action = Some(ToastAction {
            label: label.into(),
            on_click,
        });
        self
    }

    /// Override the auto-close duration set by [`Toast::new`].
    #[allow(dead_code)] // part of the public builder API; not every caller
    // needs a custom duration
    pub fn auto_close(mut self, after: Duration) -> Self {
        self.auto_close = Some(after);
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
        let run = move || {
            let ctx = expect_context::<ToastCtx>();
            let id = next_toast_id();
            let auto_close = self.auto_close;
            let entry = Entry {
                id,
                message: self.message,
                action: self.action,
                on_dismiss: self.on_dismiss,
            };
            ctx.0.update(|toasts| toasts.push(entry));
            if let Some(duration) = auto_close {
                set_timeout(move || dismiss_toast(id), duration);
            }
        };
        match self.owner {
            Some(owner) => owner.with(run),
            None => run(),
        }
    }
}

#[derive(Clone)]
struct Entry {
    id: u64,
    message: String,
    action: Option<ToastAction>,
    on_dismiss: Option<Callback<()>>,
}

#[derive(Clone)]
struct ToastAction {
    label: String,
    on_click: Callback<()>,
}

#[derive(Clone, Copy)]
pub struct ToastCtx(RwSignal<Vec<Entry>>);

static TOAST_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_toast_id() -> u64 {
    TOAST_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub fn provide_toast_context() {
    provide_context(ToastCtx(RwSignal::new(Vec::new())));
}

fn dismiss_toast(id: u64) {
    let Some(ctx) = use_context::<ToastCtx>() else {
        return;
    };
    let mut on_dismiss = None;
    ctx.0.update(|toasts| {
        if let Some(pos) = toasts.iter().position(|entry| entry.id == id) {
            on_dismiss = toasts[pos].on_dismiss;
            toasts.remove(pos);
        }
    });
    if let Some(callback) = on_dismiss {
        callback.run(());
    }
}

#[component]
pub fn ToastContainer() -> impl IntoView {
    let ctx = expect_context::<ToastCtx>();
    view! {
        <div class="toast-container">
            <For
                each=move || ctx.0.get()
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
        action,
        ..
    } = entry;
    let dismiss_aria = move_tr!("toast-dismiss");
    view! {
        <div class="toast">
            <span class="toast-message">{message}</span>
            {action.map(|action| {
                let on_click = action.on_click;
                view! {
                    <button
                        class="toast-action"
                        on:click=move |_| {
                            on_click.run(());
                            dismiss_toast(id);
                        }
                    >
                        {action.label}
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
