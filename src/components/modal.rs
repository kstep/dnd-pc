use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::components::icon::Icon;

#[component]
pub fn Modal(
    show: RwSignal<bool>,
    #[prop(into)] title: Signal<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let modal_ref = NodeRef::<leptos::html::Div>::new();

    // Auto-focus first [autofocus] element when opened
    Effect::new(move || {
        if show.get()
            && let Some(el) = modal_ref.get()
            && let Ok(Some(target)) = el.query_selector("[autofocus]")
            && let Some(focusable) = target.dyn_ref::<web_sys::HtmlElement>()
        {
            let _ = focusable.focus();
        }
    });

    let close = move |_: web_sys::MouseEvent| show.set(false);

    view! {
        <Show when=move || show.get()>
            <div class="modal-overlay" on:click=close>
                <div
                    class="modal"
                    node_ref=modal_ref
                    on:click=move |ev: web_sys::MouseEvent| ev.stop_propagation()
                >
                    <div class="modal-header">
                        <span>{move || title.get()}</span>
                        <button type="button" class="modal-close" on:click=close>
                            <Icon name="x" size=20 />
                        </button>
                    </div>
                    {children()}
                </div>
            </div>
        </Show>
    }
}
