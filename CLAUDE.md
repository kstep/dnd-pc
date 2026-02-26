# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
trunk serve --port 3000 --open   # Dev server with hot reload
trunk build --release             # Production build
cargo clippy                      # Lint
cargo +nightly fmt                # Format (requires nightly — uses edition 2024 rustfmt features)
```

Deployment to GitHub Pages uses `trunk build --release --public-url /dnd-pc/`. CI is in `.github/workflows/deploy.yml`.

## Architecture

Leptos 0.8 CSR (client-side rendered) PWA targeting `wasm32-unknown-unknown`, bundled with Trunk.

### Routing (`src/lib.rs`)
- `/` — Character list (create, delete, select)
- `/c/:id` — Character sheet editor (loads by UUID, auto-saves)
- `/s/:data` — Import shared character from compressed URL

### Reactive State (`reactive_stores`)
`Store<Character>` is the core state container. All model structs in `src/model/` derive `Store`, which generates `{Name}StoreFields` traits for field-level reactivity.

**Providing & consuming:**
```rust
// In character_sheet.rs — provides store to all child components
let store = Store::new(character);
provide_context(store);

// In any component
let store = expect_context::<Store<Character>>();
```

**Field access patterns:**
- Simple fields: `store.identity().name().get()` / `.set()` / `.update(|v| ...)`
- Vec fields: `store.features().read()` for iteration, `.write()` for mutation
- HashMap fields: `store.skills().update(|m| { ... })` — use `.update()` to avoid temporary borrow issues
- Computed values: `Memo::new(move |_| store.get().initiative())`
- `Show when=` requires a closure: `move || memo.get()`, not a raw Memo

**Auto-save:** An `Effect::new()` in `character_sheet.rs` watches `store.get()` (tracks root) and saves to localStorage on any change.

### Storage (`src/storage.rs`)
Uses `gloo_storage::LocalStorage`. Character index (list of summaries) stored at `dnd_pc_index`, individual characters at `dnd_pc_char_{uuid}`. Saving a character also updates the index.

### Character Sharing (`src/share.rs`)
Pipeline: `Character` → strip descriptions/temps → `postcard` binary serialize → `brotli` compress (quality 11) → `base64` URL-safe no-pad encode. Decode reverses the pipeline. Character UUID is preserved for future sync.

### Rules Registry (`src/rules.rs`)
`RulesRegistry` is provided as context at the App root. Class definitions (JSON files in `public/classes/`) are lazily fetched via `LocalResource` and cached in a `RwSignal<HashMap>`. The `apply_level()` logic in `character_header.rs` applies class features, proficiencies, spell slots, and HP on level-up.

### Enums (`src/model/enums.rs`)
All enums use `#[repr(u8)]` with a custom `enum_serde_u8!` macro for compact serialization (single byte) while accepting legacy string format on deserialization. Enums implement `Translatable` trait for i18n keys.

### i18n
Uses `leptos-fluent` with Fluent `.ftl` files in `locales/{en,ru}/main.ftl`. Language detected from browser, persisted in localStorage.

## Formatting Conventions (rustfmt.toml)
- Edition 2024 formatting rules
- `imports_granularity = "Crate"` — merge imports from the same crate
- `group_imports = "StdExternalCrate"` — std first, then external, then local
- `merge_derives = false` — keep separate derive attributes as-is

## Model Essentials
All model structs derive `Store`, `Clone`, `Debug`, `Serialize`, `Deserialize`, `PartialEq` (PartialEq is required for Memo). Key computed methods live on `Character`: `level()`, `proficiency_bonus()`, `ability_modifier()`, `skill_bonus()`, `initiative()`, `spell_save_dc()`, `spell_attack_bonus()`.
