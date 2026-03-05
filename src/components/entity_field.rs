use leptos::prelude::*;

use crate::{
    BASE_URL,
    components::{datalist_input::DatalistInput, icon::Icon},
};

#[component]
pub fn EntityField(
    #[prop(into)] name: Signal<String>,
    #[prop(into)] applied: Signal<bool>,
    options: Memo<Vec<(String, String, String)>>,
    #[prop(into)] ref_prefix: &'static str,
    #[prop(into)] apply_title: Signal<String>,
    #[prop(into)] placeholder: Signal<String>,
    on_input: impl Fn(String) + Copy + Send + Sync + 'static,
    fetch: impl Fn(&str) + Copy + Send + Sync + 'static,
    has: impl Fn(&str) -> bool + Copy + Send + Sync + 'static,
    apply: impl Fn(&str) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    move || {
        let current_name = name.get();
        let is_applied = applied.get();

        let display = options
            .read()
            .iter()
            .find(|(n, _, _)| *n == current_name)
            .map(|(_, label, _)| label.clone())
            .unwrap_or_else(|| current_name.clone());

        if !current_name.is_empty() {
            fetch(&current_name);
        }

        let show_apply = has(&current_name) && !is_applied;

        view! {
            <div class="entity-input-row">
                <DatalistInput
                    value=display
                    placeholder=placeholder
                    options=options
                    ref_href=move || {
                        let key = name.get();
                        (!key.is_empty()).then(|| format!("{BASE_URL}/r/{ref_prefix}/{key}"))
                    }
                    on_input=move |input, resolved| {
                        on_input(resolved.unwrap_or(input));
                    }
                />
                {if show_apply {
                    Some(view! {
                        <button
                            class="btn-apply-level"
                            title=apply_title
                            on:click=move |_| {
                                apply(&name.get_untracked());
                            }
                        >
                            <Icon name="arrow-up" size=14 />
                        </button>
                    })
                } else {
                    None
                }}
            </div>
        }
    }
}
