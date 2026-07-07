use leptos::prelude::*;

use crate::components::icon::Icon;

#[component]
pub fn HintBanner(
    #[prop(into)] icon: Signal<&'static str>,
    #[prop(into)] visible: Signal<bool>,
    #[prop(optional, into)] action_label: Option<Signal<String>>,
    #[prop(optional, into)] on_action: Option<Callback<()>>,
    /// Extra class name applied to the banner root (e.g. for per-page sizing).
    #[prop(optional, into)]
    class: Option<&'static str>,
    children: ChildrenFn,
) -> impl IntoView {
    let action = action_label.zip(on_action);
    let root_class = format!("hint-banner {}", class.unwrap_or_default());
    view! {
        <Show when=move || visible.get()>
            <div class=root_class.clone()>
                <Icon name=icon size=24 />
                <div class="hint-banner-body">{children()}</div>
                {action
                    .map(|(label, callback)| {
                        view! {
                            <button class="hint-banner-btn" on:click=move |_| callback.run(())>
                                {move || label.get()}
                            </button>
                        }
                    })}
            </div>
        </Show>
    }
}
