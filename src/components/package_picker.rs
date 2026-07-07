use leptos::prelude::*;
use leptos_fluent::{move_tr, tr};
use reactive_stores::Store;

use crate::{
    components::icon::Icon,
    model::{Character, CharacterStoreFields},
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

/// Toggle `id` in `current`, then reorder to manifest order. Pure toggle
/// math split out from the click handler so it's testable without mounting.
fn toggled_set(current: &VecSet<String>, id: &str, manifest_ids: &[String]) -> VecSet<String> {
    let mut set = current.clone();
    set.toggle(id.to_string());
    normalize_set(manifest_ids, &set)
}

/// Untracked manifest package ids in override-priority order. Empty until
/// the manifest resolves — `normalize_set` degrades to identity order on an
/// empty list, so callers don't need a separate fallback branch.
fn manifest_ids_untracked(registry: RulesRegistry) -> Vec<String> {
    registry
        .with_manifest_untracked(|entries| entries.iter().map(|entry| entry.id.clone()).collect())
        .unwrap_or_default()
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
                        class="slot-box package-chip highlighted is-unknown"
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

    let locked = Memo::new(move |_| match guard {
        Some(store) => store.with(|character| registry.locked_packages(character)),
        None => Default::default(),
    });

    let toggle = move |id: String| {
        let set = value.get_untracked();
        let ids = manifest_ids_untracked(registry);
        on_change.run(toggled_set(&set, &id, &ids));
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
                            let ids = manifest_ids_untracked(registry);
                            on_change.run(normalize_set(&ids, &rebuilt));
                        }
                    >
                        {bases
                            .iter()
                            .map(|base| {
                                let selected = value.read().contains(base.id.as_str());
                                view! {
                                    <option value=base.id.clone() selected=selected>
                                        {if compact { base.id.clone() } else { base.name.clone() }}
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
                                    class="slot-box package-chip"
                                    class:highlighted=is_on
                                    class:is-locked={
                                        let lock_names = lock_names.clone();
                                        move || lock_names().is_some()
                                    }
                                    on:click={
                                        let lock_names = lock_names.clone();
                                        move |_| {
                                            if lock_names().is_none() {
                                                toggle(click_id.clone());
                                            }
                                        }
                                    }
                                    title={
                                        let lock_names = lock_names.clone();
                                        let full_name = addon.name.clone();
                                        move || match lock_names() {
                                            Some(names) => {
                                                Some(tr!("package-locked", { "names" => names.join(", ") }))
                                            }
                                            None if compact => Some(full_name.clone()),
                                            None => None,
                                        }
                                    }
                                >
                                    {if compact { addon.id.clone() } else { addon.name.clone() }}
                                    {
                                        let lock_names = lock_names.clone();
                                        move || {
                                            lock_names()
                                                .is_some()
                                                .then(|| view! { <Icon name="lock" size=12 /> })
                                        }
                                    }
                                </button>
                            }
                        })
                        .collect_view()}
                    {unknown_chips(known, value, on_change)}
                </div>
            }
        })
    }
}

/// Package-picker panel wired to a character's `packages` store field, wrapped
/// in a page-specific container class (quick-start section vs build-tab
/// panel). Both call sites share identical label + wiring, only the wrapper
/// markup differs.
#[component]
pub fn CharacterPackagePanel(
    store: Store<Character>,
    /// DOM class for the wrapper `<div>` — differs per page.
    #[prop(into)]
    wrapper_class: String,
) -> impl IntoView {
    view! {
        <div class=wrapper_class>
            <label>{move_tr!("rule-packages")}</label>
            <PackagePicker
                value=Signal::derive(move || store.packages().get())
                on_change=Callback::new(move |set| store.packages().set(set))
                guard=store
            />
        </div>
    }
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    use super::{normalize_set, toggle_base, toggled_set};
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

    // No leptos DOM-mounting test harness is used anywhere in this codebase
    // (checked: no mount_to_body / render_to_string test pattern exists), so
    // interaction-level tests aren't feasible here. Instead the click/change
    // handlers' math is split into pure functions and unit-tested directly.

    #[wasm_bindgen_test]
    fn toggled_set_off_removes_and_normalizes() {
        let manifest_ids = ["phb24", "efoa"].map(str::to_string);
        let current: VecSet<String> = ["phb24", "efoa"].map(str::to_string).into_iter().collect();
        let result = toggled_set(&current, "efoa", &manifest_ids);
        let as_vec: Vec<&str> = result.iter().map(String::as_str).collect();
        assert_eq!(as_vec, ["phb24"]);
    }

    #[wasm_bindgen_test]
    fn toggled_set_off_unknown_id_works() {
        let manifest_ids = ["phb24"].map(str::to_string);
        let current: VecSet<String> = ["phb24", "homebrew-x"]
            .map(str::to_string)
            .into_iter()
            .collect();
        let result = toggled_set(&current, "homebrew-x", &manifest_ids);
        let as_vec: Vec<&str> = result.iter().map(String::as_str).collect();
        assert_eq!(as_vec, ["phb24"]);
    }

    #[wasm_bindgen_test]
    fn base_swap_keeps_addons() {
        let manifest_ids = ["phb24", "efoa", "motm"].map(str::to_string);
        let current: VecSet<String> = ["phb24", "efoa"].map(str::to_string).into_iter().collect();
        let bases = ["phb24".to_string(), "motm".to_string()];
        let rebuilt = toggle_base(&current, &bases, "motm");
        let normalized = normalize_set(&manifest_ids, &rebuilt);
        let as_vec: Vec<&str> = normalized.iter().map(String::as_str).collect();
        assert_eq!(as_vec, ["efoa", "motm"]);
    }
}
