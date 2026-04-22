# CLAUDE.md

Guidance for Claude Code working in this repository.

## Build & Dev Commands

```bash
trunk serve --port 3000 --open   # Dev server with hot reload
trunk build --release             # Production build
cargo clippy                      # Lint
cargo +nightly fmt                # Format (edition 2024 rustfmt features)
cargo test --lib                  # Native subset (fast — JSON validation, parsers, etc.)
WASM_BINDGEN_USE_BROWSER=1 cargo test --target wasm32-unknown-unknown --lib  # ★ Authoritative
```

**Testing rule:** this is a `wasm32-unknown-unknown` Leptos CSR PWA. The **wasm suite is the primary test signal** — most `rules::apply::*` and Leptos-integration tests use `#[wasm_bindgen_test]`, not `#[test]`, and native `cargo test` silently skips them. Always run the wasm command above before claiming green. Native `--lib` is OK as a quick sanity check alongside, never as a replacement.

Default toolchain is stable (`rust-toolchain.toml`). Nightly only needed for `fmt`.

Deploy to GitHub Pages: `trunk build --release --public-url /dnd-pc/` with `BASE_URL=/dnd-pc`. CI in `.github/workflows/deploy.yml` copies `dist/index.html` → `dist/404.html` for SPA routing.

## Architecture

Leptos 0.8 CSR PWA, `wasm32-unknown-unknown`, bundled with Trunk.

### Routing (`src/lib.rs`)
- `/` — character list
- `/c/:id` — `CharacterLayout` (ParentRoute) with nested tabs:
  - `/stats`, `/build`, `/magic`, `/inventory`, `/backstory` — `CharacterEditor` tabs
  - `/session` — game session view
  - `/quick-start` — guided character creation
  - `/story`, `/story/:story_id` — AI-generated backstory
- `/s/:user_id/:char_id` — import shared character from Firestore
- `/s/:data` — import from compressed URL (with conflict detection)
- `/r/class[/:name[/:subname]]`, `/r/species[/:name]`, `/r/background[/:name]`, `/r/feature[/:category]`, `/r/spell[/:list]` — reference browsers

Router uses `option_env!("BASE_URL")` for base path. `use_navigate()` handles the base URL internally — use plain paths like `/c/{id}`, never prepend `BASE_URL`. The `BASE_URL` constant is only for `<A href=...>` and share link construction.

### Contexts provided at App root
`RulesRegistry`, `ActiveCharacterId`, `IsRouting`, `ToastContainer`, `ArgsModalCtx`. `EffectiveCharacter` is provided per-character in `character/layout.rs`.

### Reactive State (`reactive_stores`)

`Store<Character>` is the core state container. All model structs derive `Store` (field-level reactivity).

Provide in `character/layout.rs`, consume with `expect_context::<Store<Character>>()` in child components.

**Field access:**
- Simple: `store.identity().name().get()` / `.set()` / `.update(|v| ...)`
- Vec: `.read()` for iteration, `.write()` for mutation
- HashMap: `.update(|m| { ... })` to avoid temporary borrow issues
- Computed: `Memo::new(move |_| store.get().initiative())`
- `Show when=` needs a closure: `move || memo.get()`, not a raw Memo

### Effects in `character/layout.rs`
1. **Auto-save** — persist to localStorage on any change
2. **Fill** — `registry.fill_from_registry(c)` populates empty labels/descriptions from cached JSON (re-runs on locale change)
3. **Fetch** — triggers class/species/background/spell-list fetches based on identity
4. **Effects recompute** — re-evaluates `ActiveEffects` overrides
5. **Effects save** — persists effects to separate localStorage key

### Storage (`src/storage/`)

`gloo_storage::LocalStorage`. Keys: `dnd_pc_char_{uuid}`, `dnd_pc_effects_{uuid}`, `dnd_pc_avatar_{uuid}`, `dnd_pc_stories_{uuid}`, `dnd_pc_panel_{class}`, `dnd_pc_last_sync`, `dnd_pc_last_sync_avatars`. Summaries are derived on demand by scanning keys with the `dnd_pc_char_` prefix — no separate index. Legacy `dnd_pc_index` is read once in `load_last_sync()` to seed `dnd_pc_last_sync`, then deleted. `CharacterSummary` carries `updated_at` for cheap sync comparison.

Submodules:
- `local.rs` — load/save characters, index, effects, panel state
- `sync.rs` — Firebase/Firestore sync, Google Sign-in, pull/push
- `queue.rs` — offline-first sync queue
- `migrate.rs` — sequence of migration functions (`migrate_v1`, `migrate_v2`, …) organized into **version-gated blocks**. `load_character` falls back to raw JSON + migrations on direct deserialize failure. `deserialize_character_value(Value)` applies migrations — used for cloud-fetched data. **Schema version ≠ migration count.** `CURRENT_SCHEMA_VERSION` is independent of migration-function ordinals. Inside `migrate_value`, migrations are grouped as `if version < N { … }` blocks: each block contains the steps needed to bring a character up to schema version N, and only the blocks with `version < N` run for a given input. When adding a new migration, put it in a new block gated by the next schema version (or an existing block if it logically belongs) and bump `CURRENT_SCHEMA_VERSION` accordingly. One schema version can cover many migration functions; one migration function lives in exactly one version block. All migration functions must stay idempotent (they can still run multiple times because characters may move in and out of blocks as the version history evolves).

### Character Sharing (`src/share.rs`, `src/pages/import_character.rs`)

1. **Firestore UUID** — when `character.shared == true` and authenticated: `/s/{uid}/{char_id}`. `ImportCloudCharacter` fetches via `firebase::get_character_doc()`, runs migrations, verifies `shared == true`. Firestore rules allow public read when `shared == true`.
2. **Compressed URL** — fallback. Pipeline: `strip_for_sharing(char, registry)` → postcard → `CompressionStream` (deflate-raw) → base64 url-safe → `/s/{data}`. Async due to stream API. `strip_for_sharing` uses `registry.clear_from_registry()` (selective) or falls back to `clear_all_labels()` (blanket) when registry is None.

Both import paths support conflict detection — show diff table if local UUID exists and is newer.

### Rules Registry (`src/rules/`)

`RulesRegistry` is `Copy`, provided at App root. Structural data (locale-independent) in `public/data/`, locale overlays in `public/{en,ru}/`. Overlays re-applied on language change.

Modules: `registry`, `apply`, `resolve`, `labels`, `cache`, `locale`, `index`, `class`, `species`, `background`, `feature`, `spells`, `utils`.

**Global Features Catalog:** all features live in `public/data/features.json` → `FeaturesIndex` (`BTreeMap<Box<str>, FeatureDefinition>`). Class/species/background definitions reference features by name (`VecSet<String>`).

**Feature application pipeline (`src/rules/apply/`):** `collect → gather_inputs → reapply → apply → compute`. The helper `apply_with_modal()` encapsulates the common flow: collect `ApplyInputs` from `PendingFeature` list, show args modal if needed, resolve replacements, run user callback, `compute()`. Entry points: level-up, species/background apply, quick-start (chains all three), user add, replay (combat panel). Key primitives: `PendingFeature`, `collect_class/species/background_features()`, `apply_new_features()`, `reapply_existing()`, `resolve_replacements()`, `replay()`. `FeatureDefinition::apply(level, character, when, inputs)` populates features, fields, spells, natural armor, and evaluates assignments.

**Rebuild (`apply/rebuild.rs`)** reconciles `User(_)` feature sources against identity slots and reconstructs the character. Half-migrated characters (`feature.inputs == []` but target state reflects prior picks) are recovered by the **MCV solver** (`apply/solver.rs`):

- `FeatState { def, pending, assigns: Vec<AssignData> }` — one per pending feat with interactive assigns.
- `solve_all(feats, baseline, target)` — pipeline-order recursion over `(feat_idx, assign_idx)` with backtracking. Per-assign `enumerate_assign` yields priority-ordered candidates (diff-exact → zero → brute over active slots), `passes_guard` via `eval_lenient` rejects invalid, `feat_def.apply` advances `baseline.clone_lean()` on each recursion step. Budget cap `MAX_TOTAL_ATTEMPTS = 5000` prevents runaway.
- `args_ctx::{WithArgs, WithArgsRef}` — `Context<Attribute, i32>` wrappers that intercept `@ARG(n)` lookups (mutable for apply, read-only for `eval_lenient`).
- `Character::eq_derived(&other)` — silent-commit gate; compares `abilities + saving_throws + skills + proficiencies + languages + damage_modifiers` (the derived surface feature-apply writes to). If `simulated.eq_derived(original)` passes → commit silently with toast, else open modal with partial solver prefill.
- `stored_inputs_usable(feat_def, stored)` — guards Phase 1 against half-migrated `[AssignInputs { args: [] }]` entries that would crash apply with `UnsupportedVar("ARG.0")`; unusable stored falls through to the solver.

**Key types:** `FeatureDefinition` (languages, stackable, selectable, spells, fields, assign, ac_expr, prerequisites), `FieldKind` (`Points`, `Choice`, `Die`, `Bonus`, `FreeUses`), `SpellsDefinition`, `SpellList` (`Ref { from }` or `Inline`), `ChoiceOptions` (`List` or `Ref`), `ActionType` (`Action`/`BonusAction`/`Reaction`), `Assignment { expr, when }`, `WhenCondition` (`OnFeatureAdd`/`OnLevelUp`/`OnLongRest`/`OnShortRest`/`OnCompute`).

### i18n

`leptos-fluent` with `.ftl` files in `locales/{en,ru}/main.ftl`. `move_tr!("key")` reactive, `tr!("key")` non-reactive. Language persisted in localStorage.

### Public data (`public/`)
- **Structural** (`public/data/`): `features.json`, `classes/*.json`, `species/*.json`, `backgrounds/*.json`, `spells/*.json`, `effects.json`, `index.json`, `names.json`
- **Locale overlays** (`public/{en,ru}/`): mirrored structure with labels/descriptions only

Both dirs need explicit `<link data-trunk rel="copy-dir" .../>` in `index.html`.

## Model (`src/model/`)

Split into focused files: `character`, `identity`, `ability`, `skills`, `attribute`, `attribute_group`, `feature`, `combat`, `equipment`, `spell`, `die`, `money`, `effects`, `personality`, `applied`, `enums`. All re-exported from `mod.rs`.

All structs derive `Store`, `Clone`, `Debug`, `Serialize`, `Deserialize`, `PartialEq` (required for Memo). Root `Character` omits `PartialEq`.

**Character fields:** `id`, `identity`, `abilities` (private), `saving_throws`, `skills` (private), `combat`, `personality`, `features` (container: list + data), `equipment`, `proficiencies`, `languages`, `damage_modifiers`, `spell_slots`, `applied`, `notes`, `updated_at`, `shared`. Key methods: `level()`, `proficiency_bonus()`, `initiative()`, `ability_modifier()`, `saving_throw_bonus()`, `skill_bonus()`, `spell_save_dc()`, `spell_attack_bonus()`, `caster_level()`, `active_pools()`, `class_summary()`, `clear_all_labels()`, `long_rest()`, `short_rest()`. Implements `expr::Context<Attribute>`.

**Label pattern:** `Feature`, `Spell`, `FeatureField`, `FeatureOption` have optional `label: Option<String>` (`#[serde(default)]`) with `.label()` returning `label.as_deref().unwrap_or(&name)`. `name` is the stable key; `label` is locale-filled from registry.

**Applied** (`applied.rs`): tracks build decisions already materialized. Fields: `species: bool`, `background: bool`, `levels: BTreeMap<String, VecSet<u32>>` (class → applied levels). Prevents double-application.

**Feature system:** `Features { list, data }` container. Each `Feature` has `source: FeatureSource` (`User(u32)`, `Class(String, u32)`, `Species`, `Background`, etc.), `inputs` (user choices), `applied`. Feature data (fields, spells) is stored in `Features.data: BTreeMap<String, FeatureData>`. `FeatureCategory` enum (`Class`, `Origin`, `General`, `FightingStyle`, `EpicBoon`, …) for reference browser filtering.

**Spellcasting:** Per-feature spell data in `FeatureData.spells: Option<SpellData>` keyed by feature name. Spell slots on `Character.spell_slots: BTreeMap<SpellSlotPool, ConstVec<SpellSlotLevel, 9>>` (pool = `Arcane`/`Pact`). `ClassLevel.caster_coef` (1/2/3 for full/half/third). `caster_level(pool)` sums across caster classes.

**Transient effects (`effects.rs`, `effective.rs`):** `ActiveEffect { name, description, expr, enabled }` applied via expression evaluation without modifying stored character. `ActiveEffects.recompute()` caches overrides in `BTreeMap<Attribute, i32>`. `EffectiveCharacter` (Copy reactive view over `Store<Character>` + `RwSignal<ActiveEffects>`) resolves through overrides first. Effects stored separately at `dnd_pc_effects_{uuid}` — not cloud-synced. Predefined effects in `public/data/effects.json` + locale overlay.

**Attribute** (`attribute.rs`): `Expr` variable type. Variants: `Ability`, `Modifier`, `SavingThrow`, `Skill`, `MaxHp`, `Hp`, `TempHp`, `Level`, `Ac`, `Speed`, `ClassLevel`, `CasterLevel`, `CasterModifier`, `ProfBonus`, `Initiative`, `Inspiration`. Parses dotted notation (`STR.MOD`, `SKILL.ACRO`) and reserved identifiers. Character implements `Context<Attribute>`.

## Expressions (`src/expr/`)

Generic RPN expression evaluator. `Expr<Var, Val, Grp>` (defaults: `Val = i32`, `Grp = NoGroup<Var>`). Features:
- Arithmetic (`+ - * / \ %`), dice (`2d20kh1`, `2d6dl1`), `min`/`max`, logical/comparison, `if(c,t,e)`, `guard(c){...}`
- Assignment (`var = e`), compound assignment (`+= -= *= /=`), multi-statement (semicolon)
- Loops: `each(@GROUP, body)`, `fold(op, @GROUP[, expr])`, `with(@GROUP, body)` (binds `@`), masked subgroups `@GROUP(X, Y)`
- Custom `Interpreter<Var, Val, Grp>` trait for analysis passes (`Evaluator`, `Formatter`, `DicePoolEvaluator`, `ExprAnalysis`)

Display round-trips to infix via `Formatter`. Custom deserialization accepts strings (parsed) or postfix `Vec<Op>` (postcard).

Type aliases in model: `model::Expr = Expr<Attribute, i32, AttributeGroup>`, `model::Op = Op<Attribute, i32, AttributeGroup>`.

## Utility Types
- `ConstVec<T, N>` (`src/constvec.rs`) — fixed-size vector, trims trailing defaults on serialization
- `VecSet<T>` (`src/vecset.rs`) — Vec-backed ordered set
- `Money` (`src/model/money.rs`) — copper-based currency (`u32` cp, 100 cp = 1 gp)

## Formatting (`rustfmt.toml`)
Edition 2024, `imports_granularity = "Crate"`, `group_imports = "StdExternalCrate"`, `merge_derives = false`, `normalize_comments = true`, `reorder_impl_items = true`, `wrap_comments = true`.

## Coding Conventions

### Rust style
- **Closure parameters:** descriptive names (`|character|`, `|pending|`, `|armor|`, `|assignment|`), not `|c|`, `|e|`. Exceptions: `|ch|` for character, `|cl|` for `ClassLevel` — only in one-liner closures (single-expression body). Sort/cmp closures `|a, b|` OK.
- **Imports:** always `use crate::...` (absolute paths), never `use super::...`. Single-crate project — prefer plain `pub` over `pub(crate)`; they're equivalent.
- **Leptos rendering:** don't `.to_string()` numeric types — they render directly in `view!`
- **`bind:value`:** only when signal type matches directly. For type mismatches use `prop:value` + `on:input` — don't bridge with Effects
- **Testing:** wasm is the primary test suite (`WASM_BINDGEN_USE_BROWSER=1 cargo test --target wasm32-unknown-unknown --lib`). Native `cargo test --lib` skips `#[wasm_bindgen_test]`-only tests — never the authoritative green signal.

### Locale data
- **Russian locale:** never put English placeholder text in `public/ru/` — the overlay system falls back to `en/` automatically. Omit untranslated entries.

### Postcard serialization (share pipeline)
`#[serde(flatten)]` and `#[serde(tag = "...")]` are incompatible with postcard. `FeatureField.value` uses default (externally-tagged) enum representation. Avoid `#[serde(skip_serializing)]` on postcard-serialized fields — breaks positional alignment. Label fields use `#[serde(default)]` for backward compat with older shared URLs.

### Git workflow
- One commit per logical task. Don't micro-commit.
