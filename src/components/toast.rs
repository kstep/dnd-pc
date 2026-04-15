use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use leptos::{leptos_dom::helpers::set_timeout, prelude::*};
use leptos_fluent::move_tr;

use crate::components::icon::Icon;

#[derive(Clone)]
pub struct Toast {
    pub id: u64,
    pub message: String,
    pub action: Option<ToastAction>,
    pub auto_close: Option<Duration>,
    pub on_dismiss: Option<Callback<()>>,
}

#[derive(Clone)]
pub struct ToastAction {
    pub label: String,
    pub on_click: Callback<()>,
}

#[derive(Clone, Copy)]
pub struct ToastCtx(pub RwSignal<Vec<Toast>>);

static TOAST_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn next_toast_id() -> u64 {
    TOAST_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub fn provide_toast_context() {
    provide_context(ToastCtx(RwSignal::new(Vec::new())));
}

pub fn show_toast(toast: Toast) {
    let ctx = expect_context::<ToastCtx>();
    let id = toast.id;
    let auto_close = toast.auto_close;
    ctx.0.update(|toasts| toasts.push(toast));
    if let Some(duration) = auto_close {
        set_timeout(move || dismiss_toast(id), duration);
    }
}

pub fn dismiss_toast(id: u64) {
    let Some(ctx) = use_context::<ToastCtx>() else {
        return;
    };
    let mut on_dismiss = None;
    ctx.0.update(|toasts| {
        if let Some(pos) = toasts.iter().position(|toast| toast.id == id) {
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
                key=|toast| toast.id
                let:toast
            >
                <ToastView toast />
            </For>
        </div>
    }
}

#[component]
fn ToastView(toast: Toast) -> impl IntoView {
    let Toast {
        id,
        message,
        action,
        ..
    } = toast;
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
