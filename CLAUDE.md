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
`RulesRegistry` is provided as context at the App root. Class, race, and background definitions (JSON in `public/classes/`, `public/races/`, `public/backgrounds/`) are lazily fetched via `LocalResource` and cached in `RwSignal<HashMap>` per type. Spell lists (JSON in `public/spells/`) are also lazily fetched and cached in a separate `spell_list_cache`.

**Key types:**
- `SpellsDefinition` — per-feature spellcasting config: `casting_ability`, `list` (spell list), `levels` (slot/cantrip/spell progression per character level)
- `SpellList` — `#[serde(untagged)]` enum: `Inline(Vec<SpellDefinition>)` or `Ref { from: String }` (path to JSON file)
- `SpellLevelRules` — per-level spell slot counts, cantrips known, spells known (keyed by character level in a `BTreeMap<u32, _>`)

**Key patterns:**
- `with_feature(identity, name, |feat| ...)` — finds a `FeatureDefinition` across class/background/race caches without cloning, calls the callback with a reference
- `with_spell_list(list, |spells| ...)` — resolves a `SpellList` (inline or fetched ref) and calls the callback with `&[SpellDefinition]`
- `get_for_level(levels, level)` — finds the highest `BTreeMap` key `<= level` using `.range(..=level).next_back()`

**Level-up:** `apply_level()` in `character_header.rs` applies class features, proficiencies, and HP. Spell slot progression is handled by `FeatureDefinition::apply()` which populates `character.spellcasting` (a `BTreeMap<String, SpellcastingData>` keyed by feature name).

### Enums (`src/model/enums.rs`)
All enums use `#[repr(u8)]` with a custom `enum_serde_u8!` macro for compact serialization (single byte) while accepting legacy string format on deserialization. Enums implement `Translatable` trait for i18n keys.

### i18n
Uses `leptos-fluent` with Fluent `.ftl` files in `locales/{en,ru}/main.ftl`. Language detected from browser, persisted in localStorage.

## Formatting Conventions (rustfmt.toml)
- Edition 2024 formatting rules
- `imports_granularity = "Crate"` — merge imports from the same crate
- `group_imports = "StdExternalCrate"` — std first, then external, then local
- `merge_derives = false` — keep separate derive attributes as-is

## Data Files (`public/`)
- `public/classes/*.json` — class definitions with features, levels, and `SpellsDefinition` in spellcasting features
- `public/races/*.json` — race definitions with traits and features (racial spells use `SpellsDefinition`)
- `public/backgrounds/*.json` — background definitions with features (Magic Initiate uses `SpellsDefinition`)
- `public/spells/*.json` — extracted spell lists (referenced via `SpellList::Ref { from }`)
- `public/index.json` — index of available classes, races, backgrounds

Each `public/` subdirectory needs an explicit `<link data-trunk rel="copy-dir" href="public/..." />` in `index.html` to be included in the build output.

## Model Essentials
All model structs derive `Store`, `Clone`, `Debug`, `Serialize`, `Deserialize`, `PartialEq` (PartialEq is required for Memo). Key computed methods live on `Character`: `level()`, `proficiency_bonus()`, `ability_modifier()`, `skill_bonus()`, `initiative()`, `spell_save_dc(ability)`, `spell_attack_bonus(ability)`.

**Spellcasting:** `Character.spellcasting` is a `BTreeMap<String, SpellcastingData>` keyed by feature name (e.g. "Spellcasting", "Pact Magic", "Infernal Legacy", "Magic Initiate (Wizard)"). Each entry has its own `casting_ability` and `spells` list. Spell slots are a unified pool on `Character.spell_slots: Vec<SpellSlotLevel>`, rendered once at the top of the spellcasting panel. A custom `deserialize_spellcasting` handles backward compatibility with the old `Option<SpellcastingData>` format; `Character::migrate()` merges legacy per-feature slots into the top-level pool.

**Spell slots — D&D 5e rules note:** The unified slot pool is correct for single-class characters and for racial/background spells (which don't grant slots). For multiclass casters, D&D 5e uses a combined spellcaster level table (full × 1, half × ½, third × ⅓), not a sum of individual class tables. Warlock Pact Magic slots are separate (short rest recovery). Since slot totals are editable, users can manually adjust for multiclass. Proper multiclass table calculation is not yet implemented.

**Sharing limitations (postcard):** The share pipeline uses `postcard` (positional binary format). `#[serde(flatten)]` and `#[serde(tag = "...")]` generate map serialization with unknown length, which postcard can't handle. `FeatureField` uses both, so `fields` is stripped in `strip_for_sharing`. Avoid `#[serde(skip_serializing)]` on fields of postcard-serialized structs unless the field is guaranteed empty at serialization time (it breaks positional alignment otherwise). `SpellcastingData.spell_slots` uses `skip_serializing` safely because `migrate()` always clears it before any serialization.
