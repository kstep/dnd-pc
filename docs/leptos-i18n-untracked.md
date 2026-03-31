# i18n.tr() Reactive Tracking Warnings in Leptos

## Problem

`leptos_fluent::I18n::tr()` internally reads reactive signals (`language.get()`,
`language_id_cache.with()`, `translations.with()`). These are tracked reads that
trigger a warning when called outside a reactive tracking context.

Leptos wraps every component body in `untrack_with_diagnostics` (via
`component_view`). Any `i18n.tr()` call during component construction — not
inside a `move ||` closure in the view — will warn about signal reads outside
reactive tracking.

## Anti-patterns

### Calling `.get()` on `move_tr!()`

```rust
// ❌ Wrong — .get() forces a signal read during construction
view! {
    <span>{move_tr!("some-key").get()}</span>
}

// ✅ Correct — Leptos renders the Signal reactively
view! {
    <span>{move_tr!("some-key")}</span>
}
```

`move_tr!()` returns a reactive Signal. Calling `.get()` on it during
construction evaluates the signal immediately (in the untracked context).
Without `.get()`, Leptos renders the Signal at render time (tracked context).

### Wrapping in `untrack()`

```rust
// ❌ Wrong — suppresses the warning but kills reactivity
let tr = move |key: &str| untrack(|| i18n.tr(key));
let name = attribute.display_name_with(tr);
```

`untrack()` removes the subscriber entirely, so the translation won't update
when the language changes. This is a band-aid that hides the symptom.

## Correct Patterns

### Pattern 1: Use `move ||` closures for dynamic translations

When you need `i18n.tr()` during component construction (e.g. building formula
views from expression trees), wrap the call in a `move ||` closure:

```rust
// ❌ Wrong — synchronous call during construction
let label = var.display_name(&i18n);
fb.push_text(label);

// ✅ Correct — closure evaluated at render time (tracked context)
let i18n = ctx.i18n; // I18n is Copy
fb.push_view((move || var.display_name(&i18n)).into_any());
```

The closure is not evaluated during `into_any()` — Leptos evaluates it when
mounting to the DOM (in a tracked context). The label also updates reactively
when the language changes.

### Pattern 2: Use plain `<a>` instead of `<A>` when possible

In Leptos 0.8, plain `<a href=...>` tags are intercepted by the router for SPA
navigation. `<A>` is only needed for extra features like `active_class`. If you
don't need those, plain `<a>` avoids any potential issues.

## Root Cause Details

`untrack_with_diagnostics` (from `reactive_graph`) removes the active subscriber
but sets a "diagnostics" flag. When `Track::track()` detects this flag, it emits
a console warning instead of silently ignoring the read.

A `move ||` closure in a Leptos view creates a reactive effect that is evaluated
during rendering, not during component construction. At render time, the
diagnostics subscriber is not active, so `i18n.tr()` reads signals normally
without warnings.

## Files

- `src/components/expr_args_input.rs` — `FormBuilder::exec_op` uses `move ||`
  closures for `Attribute::display_name()` calls
- `src/components/character_card.rs` — uses `move_tr!()` without `.get()`
