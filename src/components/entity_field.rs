use leptos::prelude::*;

use crate::{BASE_URL, components::datalist_input::DatalistInput};

/// Generic entity selector: DatalistInput with display name resolution and
/// reference link. No apply button — wrap in `ApplyFieldSection` for that.
#[component]
pub fn EntityField(
    /// Current entity key (e.g. class/species/background name).
    #[prop(into)]
    name: Signal<String>,
    /// Autocomplete options as `(name, label, description)` triples.
    #[prop(into)]
    options: Signal<Vec<(String, String, String)>>,
    /// URL path prefix for the reference link (e.g. `"species"`, `"class"`).
    #[prop(into)]
    ref_prefix: &'static str,
    /// Placeholder text.
    #[prop(into)]
    placeholder: Signal<String>,
    /// Whether the input is required for form validation.
    #[prop(optional)]
    required: bool,
    /// Called with the resolved name when the user makes a selection.
    on_input: impl Fn(String) + Send + Sync + 'static,
) -> impl IntoView {
    let display = Memo::new(move |_| {
        let current = name.get();
        options
            .read()
            .iter()
            .find(|(n, _, _)| *n == current)
            .map(|(_, label, _)| label.clone())
            .unwrap_or(current)
    });

    view! {
        <DatalistInput
            value=display
            placeholder=placeholder
            required=required
            options=options
            ref_href=move || {
                let key = name.get();
                (!key.is_empty()).then(|| format!("{BASE_URL}/r/{ref_prefix}/{key}"))
            }
            on_input=move |input, resolved| {
                on_input(resolved.unwrap_or(input));
            }
        />
    }
}
