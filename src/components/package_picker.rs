use leptos::prelude::*;
use leptos_fluent::tr;
use reactive_stores::Store;

use crate::{
    components::icon::Icon,
    model::Character,
    rules::{PackageKind, RulesRegistry},
    vecset::VecSet,
};

/// Reorder `set` to manifest order (base first); unknown ids keep their
/// relative order after the known ones.
fn normalize_set(manifest_ids: &[String], set: &VecSet<String>) -> VecSet<String> {
    let mut normalized: VecSet<String> = manifest_ids
        .iter()
        .filter(|id| set.contains(id.as_str()))
        .cloned()
        .collect();
    for id in set.iter() {
        if !normalized.contains(id.as_str()) {
            normalized.push(id.clone());
        }
    }
    normalized
}

/// Remove every id that is some base in `bases` from `set` and prepend
/// `new_base` — bases are mutually exclusive, `normalize_set` fixes order.
fn toggle_base(set: &VecSet<String>, bases: &[String], new_base: &str) -> VecSet<String> {
    let mut rebuilt: VecSet<String> = VecSet::new();
    rebuilt.push(new_base.to_string());
    for id in set.iter() {
        if !bases.iter().any(|base| base == id) {
            rebuilt.push(id.clone());
        }
    }
    rebuilt
}

/// Chips for ids present in the set but absent from the manifest — rendered
/// after the known ones, always toggleable off, never locked.
fn unknown_chips(
    known: Vec<String>,
    value: Signal<VecSet<String>>,
    on_change: Callback<VecSet<String>>,
) -> impl IntoView {
    move || {
        value
            .read()
            .iter()
            .filter(|id| !known.contains(id))
            .cloned()
            .map(|id| {
                view! {
                    <button
                        type="button"
                        class="entry-badge package-chip is-active is-unknown"
                        on:click=move |_| {
                            let mut set = value.get_untracked();
                            set.remove(&id);
                            on_change.run(set);
                        }
                    >
                        {id.clone()}
                    </button>
                }
            })
            .collect_view()
    }
}

#[component]
pub fn PackagePicker(
    /// Current set (order = override priority, base first).
    #[prop(into)]
    value: Signal<VecSet<String>>,
    /// Receives the full normalized set on every change.
    on_change: Callback<VecSet<String>>,
    /// Character whose content locks packages; `None` = no guard (reference).
    #[prop(optional)]
    guard: Option<Store<Character>>,
    /// Sidebar-sized chips.
    #[prop(optional)]
    compact: bool,
) -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    // Which locked chip currently shows its blockers popover.
    let open_lock: RwSignal<Option<String>> = RwSignal::new(None);

    let locked = Memo::new(move |_| match guard {
        Some(store) => store.with(|character| registry.locked_packages(character)),
        None => Default::default(),
    });

    let toggle = move |id: String| {
        let mut set = value.get_untracked();
        set.toggle(id);
        let normalized = registry
            .with_manifest_untracked(|entries| {
                let ids: Vec<String> = entries.iter().map(|entry| entry.id.clone()).collect();
                normalize_set(&ids, &set)
            })
            .unwrap_or(set);
        on_change.run(normalized);
    };

    move || {
        registry.with_manifest(|entries| {
            let bases: Vec<_> = entries
                .iter()
                .filter(|entry| entry.kind == PackageKind::Base)
                .cloned()
                .collect();
            let base_ids: Vec<String> = bases.iter().map(|base| base.id.clone()).collect();
            let addons: Vec<_> = entries
                .iter()
                .filter(|entry| entry.kind == PackageKind::Addon)
                .cloned()
                .collect();
            let known: Vec<String> = entries.iter().map(|entry| entry.id.clone()).collect();
            view! {
                <div class="package-picker" class:compact=compact>
                    <select
                        class="package-base"
                        on:change=move |event| {
                            let new_base = event_target_value(&event);
                            let set = value.get_untracked();
                            let rebuilt = toggle_base(&set, &base_ids, &new_base);
                            let normalized = registry
                                .with_manifest_untracked(|entries| {
                                    let ids: Vec<String> = entries
                                        .iter()
                                        .map(|entry| entry.id.clone())
                                        .collect();
                                    normalize_set(&ids, &rebuilt)
                                })
                                .unwrap_or(rebuilt);
                            on_change.run(normalized);
                        }
                    >
                        {bases
                            .iter()
                            .map(|base| {
                                let selected = value.read().contains(base.id.as_str());
                                view! {
                                    <option value=base.id.clone() selected=selected>
                                        {base.name.clone()}
                                    </option>
                                }
                            })
                            .collect_view()}
                    </select>
                    {addons
                        .into_iter()
                        .map(|addon| {
                            let id = addon.id.clone();
                            let is_on = {
                                let id = id.clone();
                                move || value.read().contains(id.as_str())
                            };
                            let lock_names = {
                                let id = id.clone();
                                move || locked.read().get(id.as_str()).cloned()
                            };
                            let click_id = id.clone();
                            view! {
                                <button
                                    type="button"
                                    class="entry-badge package-chip"
                                    class:is-active=is_on
                                    class:is-locked={
                                        let lock_names = lock_names.clone();
                                        move || lock_names().is_some()
                                    }
                                    on:click={
                                        let lock_names = lock_names.clone();
                                        move |_| {
                                            if lock_names().is_some() {
                                                open_lock.set(Some(click_id.clone()));
                                            } else {
                                                open_lock.set(None);
                                                toggle(click_id.clone());
                                            }
                                        }
                                    }
                                >
                                    {addon.name.clone()}
                                    {
                                        let lock_names = lock_names.clone();
                                        move || {
                                            lock_names()
                                                .is_some()
                                                .then(|| view! { <Icon name="lock" size=12 /> })
                                        }
                                    }
                                </button>
                                {move || {
                                    (open_lock.read().as_deref() == Some(id.as_str()))
                                        .then(|| {
                                            let names = lock_names().unwrap_or_default().join(", ");
                                            view! {
                                                <div
                                                    class="package-locked-pop"
                                                    on:click=move |_| open_lock.set(None)
                                                >
                                                    {tr!("package-locked", { "names" => names })}
                                                </div>
                                            }
                                        })
                                }}
                            }
                        })
                        .collect_view()}
                    {unknown_chips(known, value, on_change)}
                </div>
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    use super::normalize_set;
    use crate::vecset::VecSet;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn normalize_orders_by_manifest_and_keeps_unknown() {
        let manifest_ids = ["phb24", "efoa", "motm"].map(str::to_string);
        let set: VecSet<String> = ["motm", "homebrew-x", "phb24"]
            .map(str::to_string)
            .into_iter()
            .collect();
        let normalized = normalize_set(&manifest_ids, &set);
        let as_vec: Vec<&str> = normalized.iter().map(String::as_str).collect();
        // manifest order first, unknown ids keep their relative order at the end
        assert_eq!(as_vec, ["phb24", "motm", "homebrew-x"]);
    }
}
