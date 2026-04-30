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

### Character Sharing (`src/pages/import_character.rs`)

Only cloud sharing is supported. When `character.shared == true` and the owner is authenticated, the share link is `/s/{uid}/{char_id}`. `ImportCloudCharacter` fetches via `firebase::get_doc()`, runs migrations, and verifies `shared == true`. Firestore rules allow public read when `shared == true`.

Import supports conflict detection — shows a diff table if the local UUID exists and is newer.

### Rules Registry (`src/rules/`)

`RulesRegistry` is `Copy`, provided at App root. Structural data (locale-independent) in `public/data/`, locale overlays in `public/{en,ru}/`. Definition structs (`SpellDefinition`, `FeatureDefinition`, `ClassDefinition`, `EffectTemplate`, `*IndexEntry`, etc.) carry **only structural fields** — `name: Box<str>`, exprs, kinds. Locale text lives in a parallel resource and is overlaid via `LocalizedText` wrappers; the data resource never gets mutated on locale switch.

**Two parallel-resource patterns:**

- `LocalizedIndex<T, L>` (registry.rs): one `LocalResource<Result<T>>` for data + one `LocalResource<Option<L>>` for the locale overlay. Used for whole-file indexes (`SpellsIndex`, `FeaturesIndex`, `EffectsIndex`, `Index`). Consumers obtain `LocalizedText<'_, Def, L>` via `.lookup(name, |loc| ...)` / `.iter(...)` and read `.label()` / `.description()` on the wrapper. Locale switch reloads only the locale resource — no deep clone of N-hundred entries. Derefs to its data resource so existing `.read*()` patterns keep working.
- `LocalizedCache<T>` (cache.rs): the lazy per-name analog — pairs `FetchCache<T>` (data) + `FetchCache<LocaleMap>` (locale). Used for class/species/background definitions loaded one at a time. `DefinitionStore::lookup(name, |loc| ...)` gives a `LocalizedText<'_, Def, LocaleMap>` wrapper. On locale switch the locale cache is cleared; data survives.

**Tracked vs untracked.** Both APIs follow the leptos `signal.get()` / `get_untracked()` convention: bare `lookup` / `has` / `with` / `fetch` subscribe the calling reactive context; `_untracked` variants don't. Apply pipeline + event handlers use `_untracked`; reactive views use the bare names.

**Reactive label/description signals.** `LocalizedIndex` exposes `.label_desc(key, fallback) -> (Signal<String>, Signal<String>)` (generic over any locale map with `Borrow<str>` key) — both signals subscribe to the locale resource only. Derived signals are owned by the **calling scope**, so create them inside the `<For>` child closure (per-row scope, lives as long as the DOM node) — never inside a `Memo` body whose re-evaluation would dispose them while `<For>` keeps the matching child alive (causes "disposed reactive value" panic on locale switch).

For the shared `Index` (class/species/background/spell entries under prefixed keys), use `IndexEntry<'a>::{Class,Species,Background,Spell}(&str)` + `registry.index().entry_label_desc(entry)`. `IndexEntry` has `Display` (via strum) producing the locale-overlay key (`"class.wizard"` etc.) and `prefix()`/`name()` accessors. `EntityField`, `RefSidebarEntries`, and the per-row reference views all take a `kind: for<'a> Fn(&'a str) -> IndexEntry<'a>` constructor — pass `IndexEntry::Class` (etc.) or a `|n| IndexEntry::Class(n)` closure when the constructor doesn't infer HRTB.

`LocalizedText<'_, T, L>` derefs to `T` (transparent access to structural fields) and exposes `.label()` / `.description()` overlaying the locale entry on top of `name` fallback. Three flavors:

- Flat `L = LocaleText` (used for spells): one entry per name, simple `t.label` / `t.description` lookup.
- Nested `L = LocaleMap` for `FeatureDefinition`: bare key = feature name; `loc.field(name)` returns `LocalizedField` (key `Feat.field.X`); `field.option(name)` returns `LocalizedOption` (key `Feat.field.X.option.Y`).
- Nested `L = LocaleMap` for `ClassDefinition`: root key = `""`; `loc.subclass(name)` returns `LocalizedSubclass` (key `subclass.X`).
- Root-only for `SpeciesDefinition` / `BackgroundDefinition`: just the `""` key.

**Runtime types** (`Feature`, `Spell`, `FeatureField`, `FeatureOption`, `ActiveEffect`) keep their own `label`/`description` fields — they are user-editable and persisted with the character. `labels::sync_labels` populates them from definition lookups on every reactive cycle (Effect in `character/layout.rs`).

Modules: `registry`, `apply`, `resolve`, `labels`, `cache`, `locale`, `index`, `class`, `species`, `background`, `feature`, `spells`, `utils`.

**Global Features Catalog:** all features live in `public/data/features.json` → `FeaturesIndex` (`BTreeMap<Box<str>, FeatureDefinition>`). Class/species/background definitions reference features by name (`VecSet<String>`).

**Feature application pipeline (`src/rules/apply/`):** `collect → gather_inputs → reapply → apply → compute`. The helper `apply_with_modal()` encapsulates the common flow: collect `ApplyInputs` from `PendingFeature` list, show args modal if needed, resolve replacements, run user callback, `compute()`. Entry points: level-up, species/background apply, quick-start (chains all three), user add, replay (combat panel). Key primitives: `PendingFeature`, `collect_class/species/background_features()`, `apply_new_features()`, `reapply_existing()`, `resolve_replacements()`, `replay()`.

**Apply orchestration:** the apply method lives ONLY on `RulesRegistry`. `RulesRegistry::apply(character, feature_index, when)` (and `apply_silent`) is the single apply entry point. Internally it builds `ApplyContext { character: &mut Character, feature_index: usize, expr_index: usize }` and evaluates assignments matching `when` against the feature at `character.features.list[feature_index]`. `ApplyContext` implements `expr::Context<Attribute, i32>`: `Arg(n)` resolves through `feature.inputs[expr_index].args`; scoped/named pool variants (`Points/PointsMax/DieSides/DieCount/DieUsed/Bonus/ChoiceCount/Sticky/FreeUses/FreeUsesUsed`) lazy-create the corresponding `FeatureField`/`Spell` rows in `feature.data`; everything else delegates to `Character::resolve/assign`. `model::Feature` and `rules::FeatureDefinition` carry NO apply method — layering: `model/` is pure data, `rules/FeatureDefinition` is pure catalog, orchestration is in `RulesRegistry`. Solver and rebuild dry-runs push the trial feature into a cloned `features.list` first, then call `registry.apply` with the resulting `feature_index`.

**Rebuild (`apply/rebuild.rs`)** reconciles `User(_)` feature sources against identity slots and reconstructs the character. Half-migrated characters (`feature.inputs == []` but target state reflects prior picks) are recovered by the **MCV solver** (`apply/solver.rs`):

- `FeatState { def, pending, assigns: Vec<AssignData> }` — one per pending feat with interactive assigns.
- `solve_all(feats, baseline, target)` — pipeline-order recursion over `(feat_idx, assign_idx)` with backtracking. Per-assign `enumerate_assign` yields priority-ordered candidates (diff-exact → zero → brute over active slots), `passes_guard` via `eval_lenient` rejects invalid, `dry_run_apply_feature` advances `baseline.clone_lean()` on each recursion step (push to clone'd list, then `registry.apply`).
- `args_ctx::{WithArgs, WithArgsRef}` — `Context<Attribute, i32>` wrappers that intercept `@ARG(n)` lookups (mutable for apply, read-only for `eval_lenient`).
- `Character::eq_derived(&other)` — silent-commit gate; compares `abilities + saving_throws + skills + proficiencies + languages + damage_modifiers` (the derived surface feature-apply writes to). If `simulated.eq_derived(original)` passes → commit silently with toast, else open modal with partial solver prefill.
- `stored_inputs_usable(feat_def, stored)` — guards Phase 1 against half-migrated `[AssignInputs { args: [] }]` entries that would crash apply with `UnsupportedVar("ARG.0")`; unusable stored falls through to the solver.

**Key types:** `FeatureDefinition` (languages, stackable, selectable, spells, actions, assign, prerequisites — note: `actions: BTreeMap<Box<str>, ActionDefinition>`, no kind discriminator since pure-assign migration), `ActionDefinition` (name, options, cost?), `SpellsDefinition`, `SpellList` (`Ref { from }` or `Inline`), `ChoiceOptions` (`List` or `Ref`), `ActionType` (`Action`/`BonusAction`/`Reaction`), `Assignment { expr, when }` (no scope — pre-migration `scope` field removed), `WhenCondition` (`OnFeatureAdd`/`OnLongRest`/`OnShortRest`/`OnCompute`).

**Pure-assign attribute scheme:** named pools and per-feature state mutate via assign expressions through `ApplyContext`. `AttrKey::{Scoped, Named(&'static str)}` addresses pools by their backtick-quoted name (e.g. `` POINTS.`Sorcery Points`.MAX = tier(CLASS.LEVEL, ...) ``). Scoped form refers to the current feature scope (one indirection less). Lazy creation: writing to a `Named` pool that doesn't exist creates the corresponding `FeatureField` (Points/Die/Bonus/Choice) or `Spell` row (Sticky/FreeUses) in the scoped feature. Cross-feature reads/writes find first match by `features.list` order.

**Migration scripts** are transient — one-shot Python files under `scripts/migrate_stage_*.py` that rewrite JSON in lockstep with code changes. Not committed; regenerate from RFC + plan if needed for re-runs.

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

**Feature system:** `Features { list, data }` container. Each `Feature` has `source: FeatureSource` (`User(u32)`, `Class(String, u32)`, `Species`, `Background`, etc.), `inputs` (user choices for ARG-bearing assigns), `applied`. Feature data (fields, spells) is stored in `Features.data: BTreeMap<String, FeatureData>`. `FeatureCategory` enum (`Class`, `Origin`, `General`, `FightingStyle`, `EpicBoon`, …) for reference browser filtering. `model::Feature` is pure data — does NOT know about `rules::FeatureDefinition`; the apply pipeline looks definitions up via `RulesRegistry`.

**Spellcasting:** Per-feature spell data in `FeatureData.spells: Option<SpellData>` keyed by feature name. Spell slots on `Character.spell_slots: BTreeMap<SpellSlotPool, ConstVec<SpellSlotLevel, 9>>` (pool = `Arcane`/`Pact`). `ClassLevel.caster_coef` (1/2/3 for full/half/third). `caster_level(pool)` sums across caster classes.

**Transient effects (`effects.rs`, `effective.rs`):** `ActiveEffect { name, description, expr, enabled }` applied via expression evaluation without modifying stored character. `ActiveEffects.recompute()` caches overrides in `BTreeMap<Attribute, i32>`. `EffectiveCharacter` (Copy reactive view over `Store<Character>` + `RwSignal<ActiveEffects>`) resolves through overrides first. Effects stored separately at `dnd_pc_effects_{uuid}` — not cloud-synced. Predefined effects in `public/data/effects.json` + locale overlay.

**Attribute** (`attribute.rs`): `Expr` variable type. Singletons follow a consistent dotted scheme: `LEVEL`, `CLASS.LEVEL`, `HP`/`HP.MAX`/`HP.TEMP`, `AC`, `SPEED`, `INIT`/`INIT.BONUS`, `INSPIRATION`, `PROF.BONUS`, `ATK`, `ATTACKS`. Spellcasting: `CASTER.LEVEL[.ARCANE/.PACT]`, `CASTER.MOD`/`CASTER.ABILITY`/`CASTER.COEF`, `SLOT.<n>`/`SLOT.<n>.USED`, `SLOT.LEVEL`, `SLOT.POOL`, `SPELL.DC`/`SPELL.ATK`/`SPELL.READY`/`SPELL.KNOWN`/`SPELL.CANTRIPS`. Per-ability: `STR`/`DEX`/...//`CHA`, with `.MOD`/`.SAVE`/`.SAVE.PROF`/`.ADV` suffixes. Skills: `SKILL.<abbr>` + `.PROF`/`.ADV`. Damage mods: `RESIST.<dt>`/`VULN.<dt>`/`IMMUNE.<dt>`/`DR.<dt>`. Maps: `LANG.\`<n>\``, `FEAT.\`<n>\``, `PROF.<equip>`, `FEAT_CAT.<cat>`. Named pools (lazy-created on first write): `POINTS.\`<n>\``/`POINTS.\`<n>\`.MAX`, `DIE.\`<n>\`.SIDES`/`.COUNT`/`.USED`, `BONUS.\`<n>\``, `CHOICE.\`<n>\`.COUNT`. Spell grants: `STICKY.\`<spell>\``, `FREE_USES.\`<spell>\``/`.USED`, scoped `FREE_USES`/`FREE_USES.USED` for pool broadcast. ARG: `ARG(n)` resolves through ApplyContext. `Character` implements `Context<Attribute>` for reads; mutating apply goes through `ApplyContext`.

## Expressions (`src/expr/`)

Generic RPN expression evaluator. `Expr<Var, Val, Grp>` (defaults: `Val = i32`, `Grp = NoGroup<Var>`). Features:
- Arithmetic (`+ - * / \ %`), dice (`2d20kh1`, `2d6dl1`), `min`/`max`, logical/comparison, `if(c,t,e)`, `guard(c){...}`
- Assignment (`var = e`), compound assignment (`+= -= *= /=`), multi-statement (semicolon)
- Loops: `each(@GROUP, body)`, `fold(op, @GROUP[, expr])`, `with(@GROUP, body)` (binds `@`), masked subgroups `@GROUP(X, Y)`
- Custom `Interpreter<Var, Val, Grp>` trait for analysis passes (`Evaluator`, `Formatter`, `DicePoolEvaluator`, `ExprAnalysis`)

Display round-trips to infix via `Formatter`. Custom deserialization accepts strings (parsed) or postfix `Vec<Vec<Op>>`/`Vec<Op>` (legacy JSON array format).

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

### Git workflow
- One commit per logical task. Don't micro-commit.
- Commit messages: short title + at most one sentence on **why**. Don't list files, structs, function signatures, or per-piece changes — `git diff` shows that. The body explains intent (constraint, bug, user request); the diff explains content.
