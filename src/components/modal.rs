use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::components::icon::Icon;

#[component]
pub fn Modal(
    show: RwSignal<bool>,
    #[prop(into)] title: Signal<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let dialog_ref = NodeRef::<leptos::html::Dialog>::new();

    // Open/close the dialog via .showModal()/.close()
    // Closing plays a CSS animation first, then calls .close() on animationend.
    Effect::new(move || {
        let Some(dialog) = dialog_ref.get() else {
            return;
        };
        if show.get() {
            if !dialog.open() {
                dialog.class_list().remove_1("closing").ok();
                let _ = dialog.show_modal();
            }
        } else if dialog.open() {
            dialog.class_list().add_1("closing").ok();
        }
    });

    // After close animation finishes, actually close the dialog
    let on_animationend = move |_: web_sys::AnimationEvent| {
        if let Some(dialog) = dialog_ref.get()
            && dialog.class_list().contains("closing")
        {
            dialog.class_list().remove_1("closing").ok();
            dialog.close();
        }
    };

    // Browser fires "close" when Escape is pressed or .close() is called
    let on_close = move |_: web_sys::Event| {
        show.set(false);
    };

    // Click on backdrop (the dialog itself, not its content) closes
    let on_click = move |event: web_sys::MouseEvent| {
        if let Some(dialog) = event
            .target()
            .and_then(|target| target.dyn_ref::<web_sys::HtmlDialogElement>().cloned())
            && dialog.open()
        {
            show.set(false);
        }
    };

    view! {
        <dialog
            node_ref=dialog_ref
            class="modal"
            on:close=on_close
            on:click=on_click
            on:animationend=on_animationend
        >
            <div class="modal-content" on:click=move |ev: web_sys::MouseEvent| ev.stop_propagation()>
                <div class="modal-header">
                    <span>{move || title.get()}</span>
                    <button type="button" class="modal-close" on:click=move |_| show.set(false)>
                        <Icon name="x" size=20 />
                    </button>
                </div>
                {children()}
            </div>
        </dialog>
    }
}
