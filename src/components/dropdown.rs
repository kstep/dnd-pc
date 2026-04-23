use leptos::{prelude::*, slot};

#[slot]
pub struct DropdownTrigger {
    children: ChildrenFn,
}

#[component]
pub fn Dropdown(
    dropdown_trigger: DropdownTrigger,
    #[prop(optional, into)] class: String,
    children: ChildrenFn,
) -> impl IntoView {
    let open = RwSignal::new(false);
    view! {
        <div
            class="dropdown-wrapper"
            on:click=move |ev| {
                // Ignore clicks bubbling from the open dropdown panel — without
                // this guard the wrapper toggle would re-open it after the
                // panel handler set open=false. We can't stop_propagation in the
                // panel handler because that would also kill leptos-router's
                // window-level click listener and turn link clicks into full
                // page reloads.
                let target = event_target::<web_sys::Element>(&ev);
                if target.closest(".dropdown").ok().flatten().is_some() {
                    return;
                }
                open.update(|v| *v = !*v);
            }
        >
            {(dropdown_trigger.children)()}
            <Show when=move || open.get()>
                <div
                    class=format!("dropdown {class}").trim().to_owned()
                    on:click=move |_| open.set(false)
                >
                    {children()}
                </div>
            </Show>
        </div>
    }
}
