use leptos::prelude::*;

static DATALIST_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn next_datalist_id() -> String {
    let id = DATALIST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("datalist-{id}")
}

/// A text input with an associated `<datalist>` for autocomplete suggestions.
///
/// Each instance generates a unique datalist ID internally.
#[component]
pub fn DatalistInput(
    /// Current input value
    #[prop(into)]
    value: String,
    /// Placeholder text
    #[prop(into)]
    placeholder: String,
    /// CSS class for the input
    #[prop(into, optional)]
    class: Option<String>,
    /// Autocomplete options as `(value, label)` pairs.
    /// If label is empty, only the value is shown.
    #[prop(into)]
    options: Vec<(String, String)>,
    /// Called with the new value on each input event
    on_input: impl Fn(String) + 'static,
) -> impl IntoView {
    let datalist_id = next_datalist_id();
    let list_id = datalist_id.clone();

    view! {
        <datalist id=datalist_id>
            {options.into_iter().map(|(val, label)| {
                if label.is_empty() {
                    view! { <option value=val /> }.into_any()
                } else {
                    view! { <option value=val>{label}</option> }.into_any()
                }
            }).collect_view()}
        </datalist>
        <input
            type="text"
            class=class.unwrap_or_default()
            list=list_id
            placeholder=placeholder
            prop:value=value
            on:input=move |e| {
                on_input(event_target_value(&e));
            }
        />
    }
}
