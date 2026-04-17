# Entry Name Click-To-Toggle

## Goal

Clicking on `<span class="entry-name">` expands/collapses the surrounding `.entry-item`, matching the behavior of the existing `ToggleButton`. Editable name fields (`<input>`, `<select>`, `<label>` with `class="entry-name"`) remain untouched — clicking them focuses for editing, not expansion.

## Scope

Applies wherever `entry-name` is rendered as a `<span>`:

- `SessionList` items (spells, weapons, choices)
- Backpack read-only items
- Feature option rows (`feature_field_row.rs`)
- Damage modifiers (no-op, since those entries have no `.entry-desc`)

Inputs/selects/labels with `class="entry-name"` are **not** affected — their existing edit-on-click behavior is preserved.

## Behavior

Clicking a `span.entry-name`:

1. Find the nearest ancestor `.entry-item`.
2. If that element contains a descendant `.entry-desc`, toggle its `.expanded` class.
3. Otherwise, do nothing (matches disabled `ToggleButton` state).

Visual affordance: cursor becomes `pointer` only on name spans inside an entry-item that has a `.entry-desc` descendant.

## Implementation

### `EntryName` component (`src/components/entry_name.rs`)

```rust
use leptos::prelude::*;

#[component]
pub fn EntryName(children: Children) -> impl IntoView {
    view! {
        <span
            class="entry-name"
            on:click=move |e| {
                let span: web_sys::HtmlElement = event_target(&e);
                let Ok(Some(entry)) = span.closest(".entry-item") else { return };
                if entry.query_selector(":scope > .entry-desc").ok().flatten().is_some() {
                    let _ = entry.class_list().toggle("expanded");
                }
            }
        >
            {children()}
        </span>
    }
}
```

Mirrors `ToggleButton`: DOM-manipulation via `closest` + `class_list().toggle`, no reactive state.

### Call-site replacement

Replace every `<span class="entry-name">…</span>` with `<EntryName>…</EntryName>` in:

- `src/components/session_list.rs`
- `src/components/session/backpack.rs` (read-only list, not the add form)
- `src/components/session/damage_modifiers.rs`
- `src/components/feature_field_row.rs` (the span branch)

Leave `<input class="entry-name">`, `<select class="entry-name">`, `<label class="entry-name">` untouched.

### CSS (`public/styles.scss`)

Append under the existing `.entry-name` rules:

```scss
.entry-item:has(> .entry-desc) > .entry-content > span.entry-name {
  cursor: pointer;
}
```

Only spans inside an entry-item with an `.entry-desc` sibling get the pointer cursor, matching the runtime toggle guard.

## Non-goals

- No reactive signal for the expanded state (keeps parity with `ToggleButton`).
- No keyboard activation (matches current `ToggleButton` — it is also click-only).
- No changes to edit-form entry-items (effects form, equipment, feature rows, spellcasting inputs).
