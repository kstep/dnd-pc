use leptos::prelude::*;

use crate::{
    components::datalist::{DatalistInput, DatalistOption, SharedDatalist, next_datalist_id},
    rules::IndexEntry,
};

/// Generic entity selector: DatalistInput with display name resolution and
/// reference link.
#[component]
pub fn EntityField<K>(
    /// Current entity key (e.g. class/species/background name).
    #[prop(into)]
    name: Signal<String>,
    /// Autocomplete options.
    #[prop(into)]
    options: Signal<Vec<DatalistOption>>,
    /// `IndexEntry` constructor for the reference link
    /// (`IndexEntry::Class`, `IndexEntry::Species`, `IndexEntry::Background`).
    kind: K,
    /// Placeholder text.
    #[prop(into)]
    placeholder: Signal<String>,
    /// Whether the input is required for form validation.
    #[prop(optional)]
    required: bool,
    /// Called with the resolved name when the user makes a selection.
    on_input: impl Fn(String) + Send + Sync + 'static,
) -> impl IntoView
where
    K: for<'a> Fn(&'a str) -> IndexEntry<'a> + Copy + Send + Sync + 'static,
{
    let display = Memo::new(move |_| {
        let current = name.get();
        options
            .read()
            .iter()
            .find(|opt| opt.name == current)
            .map(|opt| opt.label.get())
            .unwrap_or(current)
    });

    let list_id = next_datalist_id();
    view! {
        <SharedDatalist id=list_id.clone() options=options />
        <DatalistInput
            value=display
            placeholder=placeholder
            required=required
            list_id=list_id
            options=options
            ref_href=move || {
                let key = name.get();
                (!key.is_empty()).then(|| {
                    let entry = kind(&key);
                    format!("/r/{}/{key}", entry.prefix())
                })
            }
            on_input=move |input, resolved| {
                on_input(resolved.unwrap_or(input));
            }
        />
    }
}
