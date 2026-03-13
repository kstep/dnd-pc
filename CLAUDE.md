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
- `/c/:id` — `ParentRoute` → `CharacterLayout` with nested:
  - `""` → `CharacterSheet` (3-column editor grid)
  - `"/summary"` → `CharacterSummary` (read-only summary view)
- `/s/:user_id/:char_id` — Import shared character from Firestore (UUID-based sharing)
- `/s/:data` — Import shared character from compressed URL (with conflict detection)
- `/r/class`, `/r/class/:name`, `/r/class/:name/:subname` → `ClassReference`
- `/r/race`, `/r/race/:name` → `RaceReference`
- `/r/background`, `/r/background/:name` → `BackgroundReference`
- `/r/spell`, `/r/spell/:list` → `SpellReference`

Router uses `option_env!("BASE_URL")` for base path. `lib.rs` also defines `use_theme()` for dark/light mode detection via `window.matchMedia`.

**Component hierarchy:** `App()` calls `provide_i18n_context()`, `provide_meta_context()`, provides `RulesRegistry::new(i18n)` as context, calls `storage::init_sync()`, and renders `LanguageSwitcher`, `SyncIndicator`, and `Router`.

**Navigation:** `use_navigate()` from `leptos_router` handles the base URL internally. Always use plain paths like `/c/{id}` — do NOT prepend `{BASE_URL}`. The `BASE_URL` constant is only needed for `<A href=...>` links and manual URL construction (e.g. share links with `window.location.origin`).

### Reactive State (`reactive_stores`)
`Store<Character>` is the core state container. All model structs in `src/model/` derive `Store`, which generates `{Name}StoreFields` traits for field-level reactivity.

**Providing & consuming:**
```rust
// In pages/character/layout.rs — provides store to all child components
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

**Effects in `character/layout.rs`:**
1. **Auto-save:** `store.track()` then `store.update_untracked(storage::save_character)` to save to localStorage on any change.
2. **Description fill:** `store.update(|c| registry.fill_from_registry(c))` to auto-populate empty labels and descriptions from locale-aware JSON definitions.
3. **Locale change:** Detects language changes and calls `c.clear_all_labels()`, which triggers the fill effect to re-populate from the new locale's data.
4. **Cloud sync pull:** `storage::track_cloud_character()` reloads the character from localStorage when `sync_index_version` is bumped and the cloud version is newer than the local `updated_at`.
5. **Effects recompute:** Watches `store.read()` and recomputes `ActiveEffects` overrides when character data changes.
6. **Effects auto-save:** Watches `effects.read()` and persists to localStorage via `storage::save_effects()`.

Character pages live in `src/pages/character/` (`layout.rs`, `list.rs`, `sheet.rs`, `summary.rs`).

### Storage (`src/storage.rs`)
Uses `gloo_storage::LocalStorage`. Character index (list of summaries) stored at `dnd_pc_index`, individual characters at `dnd_pc_char_{uuid}`, transient effects at `dnd_pc_effects_{uuid}`. `CharacterSummary` includes `updated_at` for cheap timestamp comparison during sync (avoids full character deserialization). Saving a character calls `touch()` (sets `updated_at`) and updates the index. Panel open/closed state persisted at `dnd_pc_panel_{class}`. Effects are loaded/saved via `load_effects(id)` / `save_effects(id, effects)` — stored separately from character data (not cloud-synced). A `SAVE_IN_FLIGHT` flag lets `track_cloud_character` suppress the auto-save effect when writing cloud-pulled data to the Store, preventing redundant re-push.

**Migration:** `load_character` first tries direct deserialization. On failure, falls back to raw JSON parsing with migrations, then deserializes the patched value:
- `migrate_v1()` — converts legacy string `damage_type` values to `DamageType` enum u8 representation
- `migrate_v2()` — converts flat `spell_slots` array to `BTreeMap<SpellSlotPool, ...>` keyed by pool
- `migrate_v3()` — converts string `Weapon.attack_bonus` to `i32`
- `migrate_v4()` — converts string `FeatureValue::Die` to structured `{ die, used }` object

`deserialize_character_value(value: Value) -> Option<Character>` applies all migrations to a `serde_json::Value` and deserializes. Used for cloud-fetched data (both sync and UUID-based sharing imports).

**Cloud sync (`src/firebase.rs`):** Firebase/Firestore integration for cross-device character sync. Firebase JS SDK is loaded from CDN in `index.html` and exposed as `window.__firebase`. Key elements:
- `SyncStatus` enum: `Disabled`, `Connecting`, `Synced`, `Syncing`, `Error`
- `init_sync()` — called at app startup; waits for Firebase, then does anonymous auth + pull
- `sign_in_with_google()` — upgrades anonymous session to persistent Google auth, then full sync
- `schedule_cloud_push(character)` — debounced (2s) push of a single character to Firestore; reads raw JSON from localStorage to avoid re-serialization
- `sync_index_version()` — reactive signal bumped after cloud pull modifies the index; `track_cloud_character()` in `character/layout.rs` watches this to reload the store when a newer version arrives
- `sync_all_with_cloud(push_local_only)` — bidirectional sync: pulls remote characters (saves remote-newer locally, pushes local-newer to cloud); when `push_local_only` is true (authenticated users via `SyncOp::FullSync`), also pushes characters that exist only locally. Uses index `updated_at` for cheap timestamp comparison before loading full characters
- `get_character_doc(uid, char_id)` — fetches a single character document from Firestore by owner UID and character UUID. Used for UUID-based public sharing

### Character Sharing (`src/share.rs`, `src/pages/import_character.rs`)

**Two sharing modes:**
1. **Firestore UUID sharing** — when `character.shared == true` and the user is authenticated, generates a short URL `/s/{uid}/{char_id}`. The `ImportCloudCharacter` component fetches the character via `firebase::get_character_doc()`, deserializes it with `storage::deserialize_character_value()` (which applies all migrations), and verifies `shared == true` before allowing import. Firestore security rules allow public read access when the document's `shared` field is `true`.
2. **Compressed URL sharing** — fallback when not authenticated or `shared` is false. Pipeline: `Character` → `strip_for_sharing(character, registry)` → `postcard` binary serialize → browser `CompressionStream` (`deflate-raw`) → `base64` URL-safe no-pad encode → `/s/{encoded_data}`. Decode reverses the pipeline using `DecompressionStream`. Character UUID is preserved for future sync. Encoding/decoding are `async` functions due to the stream-based compression API.

`strip_for_sharing` takes `registry: Option<&RulesRegistry>`. If registry is available, calls `registry.clear_from_registry()` (selectively clears only registry-matched labels/descriptions for minimal payload). Fallback: calls `character.clear_all_labels()` (blanket clear). `encode_character` also takes `registry: Option<&RulesRegistry>`.

Import page (`src/pages/import_character.rs`) handles both import types: `ImportCharacter` for compressed URLs, `ImportCloudCharacter` for Firestore UUID imports. Both support conflict detection: if the imported character's UUID already exists locally and the local copy is newer, shows a diff table (`ImportConflict`) instead of auto-importing.

### Rules Registry (`src/rules/`)
`RulesRegistry` is provided as context at the App root. `RulesRegistry::new(i18n)` takes `leptos_fluent::I18n` to enable locale-aware data fetching. Class, race, and background definitions (JSON in `public/{locale}/classes/`, `public/{locale}/races/`, `public/{locale}/backgrounds/`) are lazily fetched via `LocalResource` and cached in `RwSignal<HashMap>` per type. Spell lists (JSON in `public/{locale}/spells/`) are also lazily fetched and cached in a separate `spell_list_cache`. Caches automatically clear when the locale changes.

**Module structure:**
- `rules/registry.rs` — `RulesRegistry` struct (Copy), `DefinitionStore` accessor methods, index/cache/spell-list/effects-catalog access
- `rules/apply.rs` — `apply_class_level()`, `long_rest()`, `short_rest()`, `assign()` (rest-time expression evaluation)
- `rules/resolve.rs` — cross-cache feature lookup: `find_feature()`, `find_feature_with_source()`, `feature_class_level()`
- `rules/labels.rs` — unified `fill_from_registry()` / `clear_from_registry()` via single-traversal `sync_labels()` with closures
- `rules/cache.rs` — `FetchCache<T>` generic cache backed by `RwSignal<BTreeMap>` with dedup pending tracking; `DefinitionStore` trait + default methods (has/with/with_tracked/fetch/fetch_tracked)
- `rules/index.rs` — `Index` (private), `ClassIndexEntry`, `RaceIndexEntry`, `BackgroundIndexEntry`, `SpellIndexEntry`
- `rules/class.rs` — `ClassDefinition`, `SubclassDefinition`, `ClassLevelRules`, `SubclassLevelRules`
- `rules/race.rs` — `RaceDefinition`, `RaceTrait`, `AbilityModifier`
- `rules/background.rs` — `BackgroundDefinition`
- `rules/feature.rs` — `FeatureDefinition`, `FieldDefinition`, `FieldKind`, `ChoiceOptions`, `ChoiceOption`, `Assignment`, `WhenCondition`
- `rules/spells.rs` — `SpellsDefinition`, `SpellDefinition`, `SpellList`, `SpellMap`, `SpellLevelRules`
- `rules/utils.rs` — `get_for_level()`, `fetch_json()`
- `rules/mod.rs` — module declarations and re-exports of all public types

**Key types:**
- `SpellsDefinition` — per-feature spellcasting config: `casting_ability`, `caster_coef` (1=full, 2=half, 3=third), `list` (spell list), `cost: Option<String>` (cost field name for point-based casting), `levels: Vec<SpellLevelRules>` (indexed by class level - 1). Method `cost_info()` returns `(cost_field_name, short_suffix)` tuple. Method `apply(level, character, feature_name, source, free_uses_max)` creates SpellData, updates spell slots, adds cantrip/spell slots, handles sticky spells and free uses
- `SpellList` — `#[serde(untagged)]` enum: `Ref { from: String }` (path to JSON file) or `Inline(SpellMap)`. Default: `Inline(SpellMap::default())`. Method `ref_name()` extracts short list name from `Ref` path
- `SpellMap` — newtype around `BTreeMap<Box<str>, SpellDefinition>`, custom deserialization from JSON array via `Named` trait
- `SpellLevelRules` — per-level config: `cantrips: Option<u32>`, `spells: Option<u32>`, `slots: Option<Vec<u32>>`
- `SpellDefinition` — `name`, `label`, `level`, `description`, `sticky: bool`, `min_level: u32`
- `FeatureDefinition` — `name`, `label`, `description`, `languages: VecSet<String>`, `spells: Option<SpellsDefinition>`, `fields: BTreeMap<Box<str>, FieldDefinition>`, `assign: Option<Vec<Assignment>>` (conditional expressions)
- `FieldDefinition` — `name`, `label`, `description`, `kind: FieldKind`
- `FieldKind` — `#[serde(tag = "kind")]` enum: `Points` (with `short: Option<String>`), `Choice` (with `options`, `cost: Option<String>`), `Die`, `Bonus`, `FreeUses` — each with `levels: BTreeMap<u32, _>` for per-level progression. Method `to_value(level) -> FeatureValue` converts to model value at a given level. `FreeUses` is special: not converted to `FeatureValue`, instead sets `Spell.free_uses` during apply
- `ChoiceOptions` — `#[serde(untagged)]` enum: `List(Vec<ChoiceOption>)` or `Ref { from: String }` (references another field's choices)
- `ChoiceOption` — has `name`, `label`, `description`, `level: u32` (level-gated choices), `cost: u32` (point cost for point-based choices) fields
- `Assignment` — `{ expr: Expr<Attribute>, when: WhenCondition }` for feature expressions
- `WhenCondition` — enum: `Always`, `OnFeatureAdd`, `OnLevelUp`, `OnLongRest`, `OnShortRest`
- `ClassDefinition` — `features: BTreeMap<String, FeatureDefinition>`, `levels: Vec<ClassLevelRules>`, `subclasses: BTreeMap<String, SubclassDefinition>`, plus `label` field. Method `features(subclass)` iterates class + subclass features. Method `find_feature(name, subclass)` finds a feature by name
- `RaceDefinition` — `apply(character)` sets speed, ability mods, racial traits, features
- `BackgroundDefinition` — `apply(character)` sets ability mods, skill proficiencies, features
- All definition types have `label` field and `.label()` method returning `label.as_deref().unwrap_or(&name)`

**Custom deserializers (`src/demap.rs`):** `u32_key_map` (accepts string or numeric JSON keys for `BTreeMap<u32, V>`), `named_map` (deserializes `[{"name": ...}, ...]` arrays into `BTreeMap<String, T>` via `Named` trait).

**Key patterns:**
- `with_feature(identity, name, |feat| ...)` — finds a `FeatureDefinition` across class/subclass/background/race caches without cloning, calls the callback with a reference (delegates to `resolve::find_feature`)
- `with_spell_list(list, |spells| ...)` — resolves a `SpellList` (inline or fetched ref) and calls the callback with `&SpellMap`
- `get_for_level(levels, level)` — finds the highest `BTreeMap` key `<= level` using `.range(..=level).next_back()` (used for `FieldKind` level progressions). Lives in `rules/utils.rs`
- `feature_class_level(identity, feature_name)` — returns the class level of the class owning a feature (lives in `resolve.rs`)
- `get_choice_options(...)` — resolves `ChoiceOptions::List` or `ChoiceOptions::Ref` (dereferences another field's choices)
- `fill_from_registry(character)` — fills empty labels and descriptions from locale-aware registry definitions (lives in `labels.rs`)
- `clear_from_registry(character)` — selectively clears only labels/descriptions that match registry definitions (lives in `labels.rs`, inverse of fill)
- `long_rest(character)` / `short_rest(character)` — rest mechanics with expression evaluation for rest-triggered assignments (lives in `apply.rs`)
- `assign(character, when)` — evaluates conditional assignment expressions across all features for a given `WhenCondition` (lives in `apply.rs`)

**Level-up (in `apply.rs`):** `RulesRegistry::apply_class_level(character, class_idx, level)` is the single entry point for level-up. It applies saving throws, proficiencies, class/subclass features (single pass), HP, then re-applies race and background features at the new total level (for level-gated spells). The UI's `apply_level()` in `character_header.rs` simply calls `store.update(|c| registry.apply_class_level(c, idx, level))`. `FeatureDefinition::apply(level, character, source)` populates `character.feature_data` entries with spells (via `SpellsDefinition::apply()`), field values, and free uses. `FeatureDefinition::assign()` evaluates conditional assignment expressions (`when: WhenCondition`) against character attributes. `RulesRegistry::long_rest()` / `short_rest()` call `Character::long_rest()` / `short_rest()` then evaluate rest-triggered assignments via `assign()`.

### Enums (`src/model/enums.rs`)
All enums use `#[repr(u8)]` with a custom `enum_serde_u8!` macro for compact serialization (single byte) while accepting legacy string format on deserialization. Enums implement `Translatable` trait for i18n keys. Key enums: `Ability` (6), `Skill` (18), `Alignment` (9), `ProficiencyLevel` (None/Proficient/Expertise with `multiplier()`, `next()`, `symbol()`), `Proficiency` (6 armor/weapon types), `DamageType` (13 — has `from_name()` parser and `Translatable`), `ArmorType` (Light/Medium/Heavy), `SpellSlotPool` (Arcane/Pact with `Translatable`).

### i18n
Uses `leptos-fluent` with Fluent `.ftl` files in `locales/{en,ru}/main.ftl`. Language detected from browser, persisted in localStorage. Components use `move_tr!("key")` for reactive translations, `tr!("key")` for non-reactive.

### Pages (`src/pages/`)
- `character/list.rs` — list/create/delete characters
- `character/layout.rs` — parent route for `/c/:id`, loads character by UUID, creates `Store`, loads `ActiveEffects`, provides `Store<Character>` and `EffectiveCharacter` as context, runs 6 effects (auto-save, fill, locale, cloud sync, effects recompute, effects save), renders `<Outlet />`
- `character/sheet.rs` — renders 3-column grid with header and panels (~37 lines)
- `character/summary.rs` — read-only summary view at `/c/:id/summary` (layout with rest actions), uses components from `components/summary/`
- `import_character.rs` — handles both share URL types: `ImportCharacter` decodes compressed `/s/:data` URLs, `ImportCloudCharacter` fetches Firestore `/s/:user_id/:char_id` URLs; both use `ImportConflict` for diff table when local copy is newer
- `reference/` — class, race, background, spell reference browsers (`class.rs`, `race.rs`, `background.rs`, `spell.rs`, `sidebar.rs`); `mod.rs` contains shared view helpers (`ReferenceFeaturesView`, `FeatureChoicesView`, `FeatureSpellsView`, `collect_feature_views()`)
- `not_found.rs` — 404 page

### Components (`src/components/`)
- Top-level: `character_header`, `character_card`, `summary_header`, `summary_list`, `language_switcher`, `sync_indicator`, `datalist_input`, `ability_score_block`, `skill_row`, `toggle_button`, `icon`, `panel`, `cast_button`, `entity_field`, `resource_slot`
- `panels/`: `ability_scores`, `saving_throws`, `skills`, `combat`, `equipment`, `proficiencies`, `spellcasting`, `class_fields`, `features`, `personality`, `notes`
- `summary/`: `stats` (HP/AC/abilities/skills), `resources` (spell slots/hit dice/feature resources), `spells` (spell list with cast buttons), `weapons` (equipped weapons), `backpack` (inventory/currency with spend/gain), `choices` (feature choice fields), `languages`, `effects` (transient effects with add/toggle/edit UI)

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
- `public/{locale}/effects.json` — predefined transient effects catalog (name, description, expression)
- `public/{locale}/index.json` — index of available classes, races, backgrounds

Each locale directory needs an explicit `<link data-trunk rel="copy-dir" href="public/en" />` (and `public/ru`) in `index.html` to be included in the build output.

## Utility Types
- `ConstVec<T, N>` (`src/constvec.rs`): fixed-size vector that trims trailing defaults on serialization for compact payloads. Used for spell slot levels within each pool.
- `VecSet<T>` (`src/vecset.rs`): Vec-backed ordered set (maintains insertion order, prevents duplicates). Used for `ClassLevel.applied_levels: VecSet<u32>` and `Character.languages: VecSet<String>`.
- `Money` (`src/model/money.rs`): copper-based currency value type (`u32` cp internally, 100 cp = 1 gp). Constructors: `from_cp()`, `from_gp()`, `from_gp_cp()`, `from_gp_str()` (parses decimal input). Methods: `as_gp_sp_cp()` → `(gp, sp, cp)`. Implements `Add`, `Sub`, `Display`.
- `Expr<Var>` (`src/expr/`): generic expression evaluator module using postfix (RPN) operations. Refactored into submodules: `mod.rs` (core API, `Expr<Var>`, `Context<Var>` trait, Display), `tokenizer.rs` (lexer), `parser.rs` (recursive descent), `interpret.rs` (`Interpreter` trait + `Evaluator`/`ReadOnlyEvaluator`/`Formatter` implementations), `error.rs` (`Error` enum), `stack.rs` (evaluation stack). Supports arithmetic (`+`, `-`, `*`, `/`, `\` ceiling div, `%` modulo), dice notation (`2d20kh1`, `2d6dl1`), `min`/`max`, variable resolution via `Context<Var>` trait, assignment (`var = expr`), compound assignment (`+=`, `-=`, `*=`, `/=`, `\=`, `%=`), and multi-statement expressions (semicolon-separated). Methods: `apply()` (evaluate with assignment), `eval()` (read-only). `Display` impl round-trips to infix notation via `Formatter` interpreter. Custom deserialization accepts strings (parsed) or postfix `Vec<Op>` (for postcard). Used with `Var = Attribute` for feature and effect expressions.

## Model Essentials (`src/model/`)
Model is split into focused files: `character.rs` (Character, CharacterIndex, CharacterSummary), `identity.rs` (CharacterIdentity, ClassLevel), `ability.rs` (AbilityScores), `attribute.rs` (Attribute enum), `feature.rs` (Feature, FeatureData, FeatureField, FeatureValue, RacialTrait, FeatureSource, FeatureOption), `combat.rs` (CombatStats, SpellSlotLevel, FreeUses), `equipment.rs` (Equipment, Weapon, Item, Armor), `spell.rs` (Spell, SpellData, SpellSlotPool), `die.rs` (Die), `money.rs` (Money, Currency), `effects.rs` (ActiveEffect, ActiveEffects, EffectsIndex), `enums.rs` (all enums). All re-exported from `model/mod.rs`.

Model structs derive `Store`, `Clone`, `Debug`, `Serialize`, `Deserialize`, `PartialEq` (PartialEq is required for Memo). The root `Character` struct derives `Store`, `Clone`, `Debug`, `Serialize`, `Deserialize` (no `PartialEq`). Both `Character` and `CharacterSummary` have a `shared: bool` field (`#[serde(default)]`) that enables public Firestore sharing when `true`.

**Character field encapsulation:** Three fields are private with accessor methods: `abilities: AbilityScores` (via `ability_score()`, `ability_modifier()`, `modify_ability()`), `saving_throws: VecSet<Ability>` (via `proficient_with()`, `saving_throw_bonus()`, `update_saving_throw_proficiencies()`), `skills: BTreeMap<Skill, ProficiencyLevel>` (via `skill_proficiency()`, `skill_bonus()`, `update_skill_proficiencies()`). All other fields remain public. Additional accessor methods: `speed()`, `hp_max()`, `hp_current()`, `hp_temp()`, `armor_class()`, `gain_hp_max()`. Key computed methods: `level()`, `proficiency_bonus()`, `initiative()`, `spell_save_dc(ability)`, `spell_attack_bonus(ability)`, `caster_level(pool)`, `update_spell_slots(pool, slots)`, `spell_slot(pool, level)`, `all_spell_slots_for_pool(pool)`, `active_pools()`, `class_summary()`, `clear_all_labels()`, `long_rest()`, `short_rest()`. Character also implements `expr::Context<Attribute>` for expression evaluation (see Attribute section).

**Label/description pattern:** `Feature`, `Spell`, `RacialTrait`, `FeatureField`, and `FeatureOption` all have an optional `label: Option<String>` field (with `#[serde(default)]` for backward compatibility) and a `.label()` method that returns `label.as_deref().unwrap_or(&name)`. Labels are locale-specific display names filled from the registry; `name` is the stable key. `ClassLevel` has `class_label: Option<String>` and `subclass_label: Option<String>` with corresponding `.class_label()` / `.subclass_label()` methods. `class_summary()` uses these for display. `clear_all_labels()` blanket-clears all labels and descriptions on the character.

**Spellcasting model:** Per-feature spell data lives in `Character.feature_data: BTreeMap<String, FeatureData>` keyed by feature name (e.g. "Spellcasting (Bard)", "Pact Magic", "Infernal Legacy"). Each `FeatureData` has `source: Option<FeatureSource>`, `fields: Vec<FeatureField>`, and `spells: Option<SpellData>`. `FeatureSource` is an enum: `Class(String)`, `Race(String)`, `Background(String)`. `SpellData` contains `casting_ability: Ability`, `caster_coef: u32`, `pool: SpellSlotPool`, and `spells: Vec<Spell>`. Each `Spell` has an optional `free_uses: Option<FreeUses>` for innate casting without slots (`FreeUses { used, max }` with `available()` and `is_available()` methods). Spell slots are keyed by pool on `Character.spell_slots: BTreeMap<SpellSlotPool, ConstVec<SpellSlotLevel, 9>>`, rendered per pool in the spellcasting panel. `SpellSlotLevel { total, used }` has `available()`, `is_available()`, `is_empty()` methods.

**Feature fields:** `FeatureField { name, label, value: FeatureValue }`. `FeatureValue` is an enum: `Points { used, max }` (with `available_points()`), `Choice { options: Vec<FeatureOption> }` (with `choices()`/`choices_mut()`), `Die { die: Die, used: u32 }`, `Bonus(i32)`. `Die { amount, sides }` implements `Display` (e.g. "2d6") and `FromStr`.

**Currency:** `Currency { cp, sp, ep, gp, pp }` with `as_money() -> Money`, `gain(Money)`, `spend(Money) -> bool` methods.

**Attribute:** `Attribute` enum (`src/model/attribute.rs`) used as `Expr` variable type. Variants: `Ability(Ability)` (raw score), `Modifier(Ability)` (modifier), `SavingThrow(Ability)` (save bonus), `Skill(Skill)` (skill bonus), `MaxHp`, `Hp`, `TempHp`, `Level`, `Ac`, `Speed`, `ClassLevel`, `CasterLevel`, `CasterModifier`, `ProfBonus`, `Initiative`, `Inspiration`. Parsed from string identifiers with three forms: dotted ability notation (`STR.MOD`, `DEX.SAVE`), dotted skill notation (`SKILL.ACRO`, `SKILL.PERC`, etc. — 18 skill abbreviations), bare ability names (`STR`, `DEX` → `Ability(...)`), and reserved identifiers (`MAX_HP`, `HP`, `TEMP_HP`, `LEVEL`, `AC`, `SPEED`, `CLASS_LEVEL`, `CASTER_LEVEL`, `CASTER_MODIFIER`, `PROF_BONUS`, `INITIATIVE`, `INSPIRATION`). `Character` implements `Context<Attribute>` with assignable fields (`MaxHp`, `Hp`, `TempHp`, `Ac`, `Speed`, `Inspiration`) and read-only fields (all others). A scoped `Context<'a>` wrapper provides transient `ClassLevel`/`CasterLevel`/`CasterModifier` values during level-up and rest expression evaluation.

**Rest mechanics:** `long_rest()` restores HP to max, clears temp HP and death saves, resets hit dice used (half), resets all spell slots and free uses. `short_rest()` clears death saves and restores Pact Magic slots.

**Caster level & spell slots:** `ClassLevel.caster_coef: u8` (1=full, 2=half, 3=third) is set during level-up from the class definition. `ClassLevel.applied_levels: VecSet<u32>` tracks which class levels have been applied. `Character::caster_level(pool)` sums `level / caster_coef` across caster classes for the given pool. `update_spell_slots(pool, slots)` uses a built-in `SPELL_SLOT_TABLE` (full-caster Wizard progression) for multiclass, or the class-specific JSON slots for single-class. Slot totals are editable for manual adjustment. `active_pools()` returns pools that have any non-empty slots.

**Transient effects system (`src/model/effects.rs`, `src/effective.rs`):** Temporary character modifications (spells, items, conditions) applied via expression evaluation without modifying the stored character. `ActiveEffect { name, description, expr: Option<Expr<Attribute>>, enabled }` — individual effect. `ActiveEffects { effects: Vec<ActiveEffect>, overrides: BTreeMap<Attribute, i32> }` — container with methods: `add()`, `remove()`, `toggle()`, `update_field()`, `recompute(character)` (evaluates all enabled expressions, caches results in `overrides`), `resolve(character, attr)` (returns override or delegates to character). `overrides` is `#[serde(skip)]` — recomputed on load. `EffectsIndex` wraps `BTreeMap<Box<str>, ActiveEffect>` for catalog deserialization. `EffectiveCharacter` (`src/effective.rs`) is a Copy reactive view combining `Store<Character>` + `RwSignal<ActiveEffects>`, provided as context in `character/layout.rs`. Methods: `ability_modifier()`, `saving_throw_bonus()`, `skill_bonus()`, `proficiency_bonus()`, `armor_class()`, `speed()`, `initiative()`, `spell_save_dc()`, `spell_attack_bonus()` — all resolve through effects overrides first. Effects are stored separately from character data at `dnd_pc_effects_{uuid}` in localStorage (not cloud-synced). UI in `components/summary/effects.rs`: add form with `DatalistInput` name field (autocompletes from effects catalog, auto-fills expression and description on selection), expression input, togglable effect list with inline editing, expandable expression input with validation.

**Effects catalog (`public/{locale}/effects.json`):** Predefined effects (e.g. "Shield of Faith" → `AC += 2`, "Bladesong" → `AC += INT.MOD; SPEED += 10`, "Mage Armor" → `AC = max(AC, 13 + DEX.MOD)`). Loaded via `LocalResource` in `RulesRegistry` with `with_effects_index(|map| ...)` accessor. Locale-aware, auto-refetches on language change.

**Postcard serialization:** The share pipeline uses `postcard` (positional binary format). `#[serde(flatten)]` and `#[serde(tag = "...")]` are incompatible with postcard. `FeatureField.value` uses the default (externally-tagged) enum representation without flatten, making the `fields` map postcard-compatible and included in shared URLs. Avoid `#[serde(skip_serializing)]` on fields of postcard-serialized structs as it breaks positional alignment. Label fields use `#[serde(default)]` for backward compatibility with older shared URLs.
