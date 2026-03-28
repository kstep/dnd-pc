use leptos::prelude::*;

use crate::components::icon::Icon;

/// Wraps a field component with a label and a conditional apply button.
/// Used in CharacterHeader to add apply functionality around
/// `SpeciesField`/`BackgroundField`.
#[component]
pub fn ApplyFieldSection(
    /// Section label text.
    #[prop(into)]
    label: Signal<String>,
    /// Whether the entity has been applied.
    #[prop(into)]
    applied: Signal<bool>,
    /// Whether the entity definition is loaded and ready to apply.
    #[prop(into)]
    ready: Signal<bool>,
    /// Apply button tooltip.
    #[prop(into)]
    apply_title: Signal<String>,
    /// CSS class for the wrapper div.
    #[prop(into, optional)]
    class: Option<String>,
    /// Called when the apply button is clicked.
    on_apply: impl Fn() + Copy + Send + Sync + 'static,
    children: Children,
) -> impl IntoView {
    let wrapper_class = format!("header-field {}", class.unwrap_or_default());

    view! {
        <div class=wrapper_class>
            <label>{move || label.get()}</label>
            <div class="entity-input-row">
                {children()}
                {move || {
                    let show = ready.get() && !applied.get();
                    show.then(|| {
                        view! {
                            <button
                                class="btn-apply-level"
                                title=move || apply_title.get()
                                on:click=move |_| on_apply()
                            >
                                <Icon name="arrow-up" size=14 />
                            </button>
                        }
                    })
                }}
            </div>
        </div>
    }
}
