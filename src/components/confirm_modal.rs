use leptos::prelude::*;
use leptos_fluent::move_tr;

use crate::components::modal::Modal;

#[component]
fn ConfirmModal(
    show: RwSignal<bool>,
    #[prop(into)] title: Signal<String>,
    #[prop(into)] message: Signal<String>,
    on_confirm: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let do_confirm = move |_| {
        show.set(false);
        on_confirm();
    };

    view! {
        <Modal show title>
            <div class="modal-body">
                <p>{move || message.get()}</p>
            </div>
            <div class="modal-actions">
                <button on:click=move |_| show.set(false)>
                    {move_tr!("btn-cancel")}
                </button>
                <button class="btn-danger" on:click=do_confirm>
                    {move_tr!("btn-confirm")}
                </button>
            </div>
        </Modal>
    }
}

#[component]
pub fn ConfirmButton(
    #[prop(into)] class: String,
    #[prop(into)] title: Signal<String>,
    #[prop(into)] confirm_title: Signal<String>,
    #[prop(into)] confirm_message: Signal<String>,
    on_confirm: impl Fn() + Copy + Send + Sync + 'static,
    children: Children,
) -> impl IntoView {
    let show = RwSignal::new(false);

    view! {
        <button
            class=class
            title=title
            on:click=move |_| show.set(true)
        >
            {children()}
        </button>
        <ConfirmModal show title=confirm_title message=confirm_message on_confirm />
    }
}
