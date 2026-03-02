# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
trunk serve --port 3000 --open   # Dev server with hot reload
trunk build --release             # Production build
cargo clippy                      # Lint
cargo +nightly fmt                # Format (requires nightly — uses edition 2024 rustfmt features)
WASM_BINDGEN_USE_BROWSER=1 cargo test --target wasm32-unknown-unknown  # Run tests in headless Chrome
```

Default toolchain is stable (`rust-toolchain.toml`). Nightly is only needed for `cargo +nightly fmt`.

Deployment to GitHub Pages uses `trunk build --release --public-url /dnd-pc/` with `BASE_URL=/dnd-pc`. CI is in `.github/workflows/deploy.yml`. The CI also copies `dist/index.html` to `dist/404.html` for SPA routing on GitHub Pages.

## Architecture

Leptos 0.8 CSR (client-side rendered) PWA targeting `wasm32-unknown-unknown`, bundled with Trunk.

### Routing (`src/lib.rs`)
- `/` — Character list (create, delete, select)
- `/c/:id` — Character sheet editor (loads by UUID, auto-saves)
- `/s/:data` — Import shared character from compressed URL (with conflict detection)

Router uses `option_env!("BASE_URL")` for base path. `lib.rs` also defines `use_theme()` for dark/light mode detection via `window.matchMedia`.

**Component hierarchy:** `App()` → `I18nProvider` → `AppInner()`. `I18nProvider` wraps `AppInner` to ensure i18n is initialized first. `AppInner()` calls `expect_context::<leptos_fluent::I18n>()` and provides `RulesRegistry::new(i18n)` as context.

**Navigation:** `use_navigate()` from `leptos_router` handles the base URL internally. Always use plain paths like `/c/{id}` — do NOT prepend `{BASE_URL}`. The `BASE_URL` constant is only needed for `<A href=...>` links and manual URL construction (e.g. share links with `window.location.origin`).

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

**Auto-save:** An `Effect::new()` in `character_sheet.rs` calls `store.track()` then `store.update_untracked(storage::save_character)` to save to localStorage on any change.

**Description fill:** A second `Effect::new()` calls `store.update(|c| registry.fill_from_registry(c))` to auto-populate empty labels and descriptions from locale-aware JSON definitions.

**Locale change:** A third `Effect::new()` detects language changes and calls `c.clear_all_labels()`, which triggers the fill effect to re-populate from the new locale's data.

### Storage (`src/storage.rs`)
Uses `gloo_storage::LocalStorage`. Character index (list of summaries) stored at `dnd_pc_index`, individual characters at `dnd_pc_char_{uuid}`. Saving a character calls `touch()` (sets `updated_at`) and updates the index. Panel open/closed state persisted at `dnd_pc_panel_{class}`.

**Migration:** `load_character` first tries direct deserialization. On failure, falls back to raw JSON parsing with `migrate_v1()` (converts legacy string `damage_type` values to `DamageType` enum u8 representation), then deserializes the patched value. Add new migrations as `migrate_v2`, etc.

### Character Sharing (`src/share.rs`)
Pipeline: `Character` → `strip_for_sharing(character, registry)` → `postcard` binary serialize → `brotli` compress (quality 11, lgwin 22) → `base64` URL-safe no-pad encode. Decode reverses the pipeline. Character UUID is preserved for future sync.

`strip_for_sharing` takes `registry: Option<&RulesRegistry>`. If registry is available, calls `registry.clear_from_registry()` (selectively clears only registry-matched labels/descriptions for minimal payload). Fallback: calls `character.clear_all_labels()` (blanket clear). `encode_character` also takes `registry: Option<&RulesRegistry>`.

Import page (`src/pages/import_character.rs`) handles conflict detection: if the imported character's UUID already exists locally and the local copy is newer, shows a diff table instead of auto-importing.

### Rules Registry (`src/rules.rs`)
`RulesRegistry` is provided as context at the App root. `RulesRegistry::new(i18n)` takes `leptos_fluent::I18n` to enable locale-aware data fetching. Class, race, and background definitions (JSON in `public/{locale}/classes/`, `public/{locale}/races/`, `public/{locale}/backgrounds/`) are lazily fetched via `LocalResource` and cached in `RwSignal<HashMap>` per type. Spell lists (JSON in `public/{locale}/spells/`) are also lazily fetched and cached in a separate `spell_list_cache`. Caches automatically clear when the locale changes.

**Key types:**
- `SpellsDefinition` — per-feature spellcasting config: `casting_ability`, `caster_coef` (1=full, 2=half, 3=third), `list` (spell list), `levels: Vec<SpellLevelRules>` (indexed by class level - 1)
- `SpellList` — `#[serde(untagged)]` enum: `Ref { from: String }` (path to JSON file) or `Inline(Vec<SpellDefinition>)`. Default: `Inline(Vec::new())`
- `SpellLevelRules` — per-level config: `cantrips: Option<u32>`, `spells: Option<u32>`, `slots: Option<Vec<u32>>`
- `SpellDefinition` — `name`, `label`, `level`, `description`, `sticky: bool`, `min_level: u32`
- `FeatureDefinition` — `name`, `label`, `description`, `spells: Option<SpellsDefinition>`, `fields: BTreeMap<String, FieldDefinition>`
- `FieldDefinition` — `name`, `label`, `description`, `kind: FieldKind`
- `FieldKind` — `#[serde(tag = "kind")]` enum: `Points`, `Choice`, `Die`, `Bonus` — each with `levels: BTreeMap<u32, _>` for per-level progression
- `ChoiceOptions` — `#[serde(untagged)]` enum: `List(Vec<ChoiceOption>)` or `Ref { from: String }` (references another field's choices)
- `ChoiceOption` — has `name`, `label`, `description` fields
- `ClassDefinition` — `features: BTreeMap<String, FeatureDefinition>`, `levels: Vec<ClassLevelRules>`, `subclasses: BTreeMap<String, SubclassDefinition>`, plus `label` field
- All definition types have `label` field and `.label()` method returning `label.as_deref().unwrap_or(&name)`

**Custom deserializers:** `u32_key_map` (accepts string or numeric JSON keys for `BTreeMap<u32, V>`), `named_map` (deserializes `[{"name": ...}, ...]` arrays into `BTreeMap<String, T>`).

**Key patterns:**
- `with_feature(identity, name, |feat| ...)` — finds a `FeatureDefinition` across class/subclass/background/race caches without cloning, calls the callback with a reference
- `with_spell_list(list, |spells| ...)` — resolves a `SpellList` (inline or fetched ref) and calls the callback with `&[SpellDefinition]`
- `get_for_level(levels, level)` — finds the highest `BTreeMap` key `<= level` using `.range(..=level).next_back()` (used for `FieldKind` level progressions)
- `feature_class_level(identity, feature_name)` — returns the class level of the class owning a feature
- `get_choice_options(...)` — resolves `ChoiceOptions::List` or `ChoiceOptions::Ref` (dereferences another field's choices)
- `fill_from_registry(character)` — fills empty labels and descriptions from locale-aware registry definitions
- `clear_from_registry(character)` — selectively clears only labels/descriptions that match registry definitions (inverse of fill)

**Level-up:** `ClassDefinition::apply_level(level, character)` applies class features, saving throws, proficiencies, `caster_coef`, and HP. A wrapper `apply_level()` in `character_header.rs` calls it via the store. `FeatureDefinition::apply()` populates `character.feature_data` entries with spells and field values.

### Enums (`src/model/enums.rs`)
All enums use `#[repr(u8)]` with a custom `enum_serde_u8!` macro for compact serialization (single byte) while accepting legacy string format on deserialization. Enums implement `Translatable` trait for i18n keys. Key enums: `Ability` (6), `Skill` (18), `Alignment` (9), `ProficiencyLevel` (None/Proficient/Expertise with `multiplier()`, `next()`, `symbol()`), `Proficiency` (6 armor/weapon types), `DamageType` (13 — has `from_name()` parser and `Translatable`), `ArmorType` (Light/Medium/Heavy).

### i18n
Uses `leptos-fluent` with Fluent `.ftl` files in `locales/{en,ru}/main.ftl`. Language detected from browser, persisted in localStorage. Components use `move_tr!("key")` for reactive translations, `tr!("key")` for non-reactive.

### Pages (`src/pages/`)
- `character_list.rs` — list/create/delete characters
- `character_sheet.rs` — loads character by UUID, creates `Store`, provides context, auto-save + description-fill effects, renders 3-column grid
- `import_character.rs` — decodes `/s/:data` share URL, conflict detection with diff table; also handles local JSON file imports
- `not_found.rs` — 404 page

## Formatting Conventions (rustfmt.toml)
- Edition 2024 formatting rules
- `imports_granularity = "Crate"` — merge imports from the same crate
- `group_imports = "StdExternalCrate"` — std first, then external, then local
- `merge_derives = false` — keep separate derive attributes as-is
- `normalize_comments = true`, `reorder_impl_items = true`, `wrap_comments = true`

## Data Files (`public/`)
Data files are locale-specific, organized under `public/{en,ru}/`:
- `public/{locale}/classes/*.json` — 13 class definitions with features, levels, subclasses, and `SpellsDefinition` in spellcasting features
- `public/{locale}/races/*.json` — 16 race definitions with traits and features (racial spells use `SpellsDefinition`)
- `public/{locale}/backgrounds/*.json` — 16 background definitions with features (Magic Initiate uses `SpellsDefinition`)
- `public/{locale}/spells/*.json` — 9 extracted spell lists (referenced via `SpellList::Ref { from }`)
- `public/{locale}/index.json` — index of available classes, races, backgrounds

Each locale directory needs an explicit `<link data-trunk rel="copy-dir" href="public/en" />` (and `public/ru`) in `index.html` to be included in the build output.

## Utility Types
- `ConstVec<T, N>` (`src/constvec.rs`): fixed-size vector that trims trailing defaults on serialization for compact payloads. Used for `spell_slots: ConstVec<SpellSlotLevel, 9>`.
- `VecSet<T>` (`src/vecset.rs`): Vec-backed ordered set (maintains insertion order, prevents duplicates). Used for `ClassLevel.applied_levels: VecSet<u32>` and `Character.languages: VecSet<String>`.

## Model Essentials
Model structs derive `Store`, `Clone`, `Debug`, `Serialize`, `Deserialize`, `PartialEq` (PartialEq is required for Memo). The root `Character` struct derives `Store`, `Clone`, `Serialize`, `Deserialize` (no `PartialEq` or `Debug`). Key computed methods live on `Character`: `level()`, `proficiency_bonus()`, `ability_modifier()`, `skill_bonus()`, `initiative()`, `spell_save_dc(ability)`, `spell_attack_bonus(ability)`, `caster_level()`, `update_spell_slots()`, `class_summary()`, `clear_all_labels()`.

**Label/description pattern:** `Feature`, `Spell`, `RacialTrait`, `FeatureField`, and `FeatureOption` all have an optional `label: Option<String>` field (with `#[serde(default)]` for backward compatibility) and a `.label()` method that returns `label.as_deref().unwrap_or(&name)`. Labels are locale-specific display names filled from the registry; `name` is the stable key. `ClassLevel` has `class_label: Option<String>` and `subclass_label: Option<String>` with corresponding `.class_label()` / `.subclass_label()` methods. `class_summary()` uses these for display. `clear_all_labels()` blanket-clears all labels and descriptions on the character.

**Spellcasting model:** Per-feature spell data lives in `Character.feature_data: BTreeMap<String, FeatureData>` keyed by feature name (e.g. "Spellcasting (Bard)", "Pact Magic", "Infernal Legacy"). Each `FeatureData` has `fields: Vec<FeatureField>` and `spells: Option<SpellData>`. `SpellData` contains `casting_ability: Ability` and `spells: Vec<Spell>`. Spell slots are a unified pool on `Character.spell_slots: ConstVec<SpellSlotLevel, 9>`, rendered once at the top of the spellcasting panel.

**Caster level & spell slots:** `ClassLevel.caster_coef: u8` (1=full, 2=half, 3=third) is set during level-up from the class definition. `ClassLevel.applied_levels: VecSet<u32>` tracks which class levels have been applied. `Character::caster_level()` sums `level / caster_coef` across all caster classes. `update_spell_slots()` uses a built-in `SPELL_SLOT_TABLE` (full-caster Wizard progression) for multiclass, or the class-specific JSON slots for single-class. Slot totals are editable for manual adjustment.

**Postcard serialization:** The share pipeline uses `postcard` (positional binary format). `#[serde(flatten)]` and `#[serde(tag = "...")]` are incompatible with postcard. `FeatureField.value` uses the default (externally-tagged) enum representation without flatten, making the `fields` map postcard-compatible and included in shared URLs. Avoid `#[serde(skip_serializing)]` on fields of postcard-serialized structs as it breaks positional alignment. Label fields use `#[serde(default)]` for backward compatibility with older shared URLs.
