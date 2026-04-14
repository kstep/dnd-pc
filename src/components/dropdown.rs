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
            on:click=move |_| open.update(|v| *v = !*v)
        >
            {(dropdown_trigger.children)()}
            <Show when=move || open.get()>
                <div
                    class=format!("dropdown {class}").trim().to_owned()
                    on:click=move |ev| {
                        ev.stop_propagation();
                        open.set(false);
                    }
                >
                    {children()}
                </div>
            </Show>
        </div>
    }
}
