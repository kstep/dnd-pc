use leptos::prelude::*;

use crate::{
    components::{icon::Icon, package_picker::PackagePicker, ref_link::Ref},
    pages::reference::encode_name,
    rules::{ActivePackages, IndexEntry, RulesRegistry},
};

#[component]
pub fn ReferenceSidebar(current_label: Signal<String>, children: ChildrenFn) -> impl IntoView {
    let active_packages = expect_context::<ActivePackages>();
    let manually_open = RwSignal::new(false);

    // Reset collapsed state when navigating to a different item
    Effect::new(move || {
        current_label.track();
        manually_open.set(false);
    });

    // Auto-expand when no current item is selected
    let open = Signal::derive(move || current_label.read().is_empty() || manually_open.get());

    view! {
        <aside class="reference-sidebar" class:open=move || open.get()>
            {move || {
                (!current_label.read().is_empty())
                    .then(|| {
                        view! {
                            <button
                                class="reference-nav-toggle"
                                on:click=move |_| manually_open.update(|open| *open = !*open)
                            >
                                {move || current_label.get()}
                                <Icon name="chevron-down" />
                            </button>
                        }
                    })
            }}
            <div class="reference-sidebar-packages">
                <PackagePicker
                    value=Signal::derive(move || active_packages.0.get())
                    on_change=Callback::new(move |set| active_packages.0.set(set))
                    compact=true
                />
            </div>
            {move || children()}
        </aside>
    }
}

/// Keyed list of reference-sidebar entries. `kind` builds an
/// `IndexEntry` from a borrowed name (typically `IndexEntry::Class` /
/// `IndexEntry::Species` / etc., or a closure wrapping one); each row's
/// label is a derived signal subscribed to the index locale resource,
/// so locale switches patch text in place without re-creating DOM
/// nodes.
#[component]
pub fn RefSidebarEntries<F>(#[prop(into)] names: Signal<Vec<String>>, kind: F) -> impl IntoView
where
    F: for<'a> Fn(&'a str) -> IndexEntry<'a> + Copy + Send + Sync + 'static,
{
    let registry = expect_context::<RulesRegistry>();
    view! {
        <For
            each=move || names.get()
            key=|name| name.clone()
            children=move |name| {
                let entry = kind(&name);
                let (label, _) = registry.entry_label_desc(entry);
                let href = format!("/r/{}/{}", entry.prefix(), encode_name(&name));
                view! {
                    <Ref href=href attr:class="reference-nav-item">
                        {move || label.get()}
                    </Ref>
                }
            }
        />
    }
}
