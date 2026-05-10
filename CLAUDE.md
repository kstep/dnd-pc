# CLAUDE.md

Guidance for Claude Code working in this repository.

## Build & Dev Commands

```bash
trunk serve --port 3000 --open   # Dev server with hot reload
trunk build --release             # Production build
cargo clippy                      # Lint
cargo +nightly fmt                # Format (edition 2024 rustfmt features)
cargo test --lib --features testing  # Native subset (fast — JSON validation, parsers, etc.)
WASM_BINDGEN_USE_BROWSER=1 cargo test --target wasm32-unknown-unknown --features testing  # ★ Authoritative
```

**Testing rule:** this is a `wasm32-unknown-unknown` Leptos CSR PWA. The **wasm suite is the primary test signal** — most `rules::apply::*` and Leptos-integration tests use `#[wasm_bindgen_test]`, not `#[test]`, and native `cargo test` silently skips them. Always run the wasm command above before claiming green. Native `--lib` is OK as a quick sanity check alongside, never as a replacement.

**`testing` cargo feature:** unlocks test-only constructors like `RulesRegistry::for_test`, `LocalizedIndex::for_test`/`await_ready` for integration tests in `tests/` (separate crate, can't see `#[cfg(test)]` items from the lib). Always run `cargo test` with `--features testing`; CI does this automatically. Production builds (`trunk build`) leave the feature off, so test-only code is gated out.

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

**Feature application pipeline (`src/rules/apply/`):** all apply paths converge on a single `cascade()` over a list of `PendingFeature`. UI helper `apply_with_modal()` collects `ApplyInputs` through the args modal and feeds `cascade` via closures; level-up / quick-start / add-feat / edit / rebuild / args_modal speculative all share this entry. Public surface: `apply_pending`, `cascade`, `dry_run_apply_feature`, `restore_user_state`, `apply_feature` (registry_ext).

**`cascade()` (primitives.rs).** Recurses identity-driven follow-ups until settled. Replacement and inputs lookup come through closures (`Fn(&FeatureKey) -> Vec<AssignInputs>` + `Fn(&str) -> Option<String>`), so callers keep their native source — frozen `ApplyInputs` map, reactive RwSignals, stored `feature.inputs`, or any composition — without eager materialization. Missing-def follow-ups silent-skip with a warn log.

**`apply_pending()` per-feature.** One single-feature applicator. Order: missing-def warn → contains-dedup → drop sibling placeholders matched by `replace_with` → `inputs_sufficient` guard → push (applied=true) + `apply_feature(OnFeatureAdd)` + identity-event handling. If `inputs_sufficient` is false (interactive feat without complete ARG/dice), the row lands `applied=false` with no assigns; modal sees it on next open and `Features::add` reuses the unapplied slot once inputs arrive.

**Apply context (`ApplyContext`).** `RulesRegistry::apply(character, feature_index, when)` builds `ApplyContext { character, feature_index, expr_index }` and runs assignments matching `when` against `character.features.at(feature_index)`. `ApplyContext` implements `expr::Context<Attribute, i32>`: `Arg(n)` resolves through `feature.inputs[expr_index].args`; scoped/named pool variants (`Points/PointsMax/DieSides/DieCount/DieUsed/Bonus/ChoiceCount/Sticky/FreeUses/FreeUsesUsed`) lazy-create the corresponding `FeatureField`/`Spell` rows in `feature.data`; everything else delegates to `Character::resolve/assign`. `model::Feature` and `rules::FeatureDefinition` carry NO apply method — `model/` is pure data, `rules/FeatureDefinition` is pure catalog. `dry_run_apply_feature` is the solver-baseline path; runs assigns force-applied (Arg → Err on missing) so solver can diff against original.

**Placeholder replacement is generic.** `apply_pending` drops any sibling at the same source whose `replace_with.matches(this_def)` is true — a System(Subclass) marker landing kicks out the `Subclass` placeholder, ASI replacement evicts the ASI placeholder, etc. The replacement row records the placeholder's name in `Feature.replaces: Option<String>` (set in `cascade`); `detect_replacement` fast-paths off this field, falling back to category/prereq heuristic for legacy rows. All swaps — feat replacements, subclass picks, species/background — flow through this one mechanism plus prerequisite filters; no separate identity-slot commit path.

**`Features` API (`model/feature.rs`).** Encapsulates the per-character feature list. Methods: `iter`/`iter_mut`, `at`/`at_mut`, `find`/`find_mut`/`find_pos`, `last_pos`, `len`/`is_empty`, `contains`/`has`/`has_category`/`is_pending`, `get_inputs`, `put(...) -> usize` (upsert by `(name, source)` slot, returns position), `push(feature) -> usize` (push owned, no slot logic), `remove`, `truncate`, `data`/`data_mut`/`split_mut`, `spell_data`/`spell_data_mut`, `reset_uses`, `clear_all_labels`. Apply-pipeline callers go through these; direct `features.list` access remains only in test setup and the niche `clean.features.list.insert(pos, _)` in `rebuild.rs::merge_preserved`.

**`DefinitionCaches` (caches.rs).** `Copy` bundle of three references to the registry's class/species/background caches. Threaded by value through the apply pipeline; obtain via `registry.with_definitions(|caches| ...)`.

**FeaturesView (`registry.rs`).** Zero-allocation view over natural features index + runtime-synthesized System(_) markers. Natural takes precedence on key collision. Obtained via `registry.with_features_index{,_untracked}(|view| ...)`.

**Rebuild (`apply/plan.rs` + `apply/rebuild.rs`).** Features categorized `FeatureCategory::System(IdentitySlot::*)` carry the canonical level-up history; `identity.classes` is a denormalized cache. The category itself is just a tag for plan reconstruction and the build-history UI — `cascade` doesn't distinguish them; their assigns (`CLASS.X.LEVEL = 1`, `SPECIES.X = 1`, …) drive identity through the same identity-event pipeline as any other feature. `level_up_plan(identity, features, registry)` returns a canonical `Vec<PendingFeature>` for `build_clean` to walk. Two strategies:

- `plan_from_markers(identity, features)` — canonical path: walks `FeatureCategory::System(IdentitySlot::*)` rows. Each `System(Class)` row at `User(N)` is one character level; `System(Subclass)` rows are source-tagged `Class(<class>, <class_level>)`. Returns `None` if rows don't cover `1..=total_class_levels`.
- `plan_from_interleaving(identity, features, registry)` — legacy fallback: walks `identity.classes` round-robin honoring multiclass prereqs against a probe character. First rebuild of a legacy char goes here; emits the `System(_)` rows, so subsequent rebuilds use the canonical path.

`build_clean` walks the plan with a `cascade()`-loop: each entry either drives `cascade(clean, [pending], inputs_for, replacement_for)` (def in index) or `apply_user_pending` (orphan). `inputs_for`/`replacement_for` close over `RebuildCtx` to read stored or modal-supplied inputs. Both plan strategies signal-free — they read only `identity` + `features.list`, no other Character fields.

Half-migrated characters (`feature.inputs == []` but target state reflects prior picks) are recovered by the **MCV solver** (`apply/solver.rs`):

- `FeatState { def, pending, assigns: Vec<AssignData> }` — one per pending feat with interactive assigns.
- `solve_all(feats, baseline, target)` — pipeline-order recursion over `(feat_idx, assign_idx)` with backtracking. Per-assign `enumerate_assign` yields priority-ordered candidates (diff-exact → zero → brute over active slots); `passes_guard` via `eval_lenient` rejects invalid; `dry_run_apply_feature` advances `baseline.clone_lean()` on each recursion step.
- `Character::eq_derived(&other)` — silent-commit gate; compares `abilities + saving_throws + skills + proficiencies + languages + damage_modifiers`. Match → commit silently with toast; else open modal with partial solver prefill.
- `stored_inputs_usable(feat_def, stored)` — guards against half-migrated `[AssignInputs { args: [] }]` entries that would crash apply.

**Speculative cascade in modals (clone-and-discard).** Args modal builds per-section snapshots and pick-watcher recomputes by calling `apply_cascade_step`: clone the prior `Character`, run unified `cascade()` against the clone with reactive closures over `ArgsModalState` (the `Copy` bundle of `args`/`dice`/`valid`/`replacements` RwSignals), discard the clone after reading. Identity-flag mutations live on the clone and die with it. Recompute callbacks (`level_up_recompute`, `quick_start_recompute`, `rebuild_recompute`) clear `snapshot.applied = Applied::default()` before `collect_pending_features` so the speculative state re-surfaces fresh class L_n features.

**Edit-flow base (feature_row.rs).** Edit modal uses `build_clean(&truncated_clone)` for its pre-edit cascade base — clone the live character, truncate at the edited feature, run the standard rebuild. Single code path for both rebuild and edit-base; no separate `build_cascade_base_before` helper. The pencil shows for any feature with interactive inputs **or** `replaces.is_some()`; in the swap case the modal opens for the placeholder (e.g. ASI) with the picker pre-set to the current swap (e.g. Lucky) and Lucky's stored inputs prefilling per-expr. `edit_inputs_modal(store, registry, placeholder_name, source, base, current_name: Option<String>)` — pass `None` for `current_name` for non-swap edits. Submit delegates to `apply_edit_to_feature(&mut Feature, placeholder, submitted, feat_index) -> Option<String>` (rename / replaces / dirty rules, unit-tested); the returned previous name lets the caller clean up `features.data` after a rename. `applied = false` triggers the `BuildReplayHint` "Rebuild" banner (`build_hints.rs:60`); user clicks Rebuild → cascade reruns, the recorded `replaces` drives `detect_replacement` so the new pick lands at the same slot.

**`replacement_prefill` semantics (`pending.rs`).** `Vec<AssignInputs>` indexed by expr position (`replacement_prefill[i]` feeds expr `i`). Empty Vec = no prefill; out-of-bounds → `AssignInputs::default()`. **No broadcast** — a short Vec leaves later exprs explicitly empty rather than silently reusing one value. AI generation passes a 1-element Vec → only `expr_0` prefilled; remaining exprs render empty for the user (older broadcast behavior masked incomplete AI prefills as "AI picked the same skill twice"). Edit-of-swap-row passes the swap's full `feature.inputs` Vec for fidelity.

**Key types:** `FeatureDefinition` (languages, stackable, selectable, spells, `actions: BTreeMap<Box<str>, ActionDefinition>`, assign, prerequisites), `ActionDefinition` (name, options, cost?), `SpellsDefinition`, `SpellList` (`Ref { from }` or `Inline`), `ChoiceOptions` (`List` or `Ref`), `ActionType` (`Action`/`BonusAction`/`Reaction`), `Assignment { expr, when }`, `WhenCondition` (`OnFeatureAdd`/`OnLongRest`/`OnShortRest`/`OnCompute`).

**Pure-assign attribute scheme.** Named pools and per-feature state mutate via assign expressions through `ApplyContext`. `AttrKey::{Scoped, Named(&'static str)}` addresses pools by their backtick-quoted name (e.g. `` POINTS.`Sorcery Points`.MAX = tier(CLASS.LEVEL, ...) ``). Scoped form refers to the current feature scope. Lazy creation: writing to a `Named` pool that doesn't exist creates the corresponding `FeatureField` (Points/Die/Bonus/Choice) or `Spell` row (Sticky/FreeUses) in the scoped feature. Cross-feature reads/writes find first match by `features.list` order.

**Migration scripts** are transient — one-shot Python files under `scripts/migrate_stage_*.py` that rewrite JSON in lockstep with code changes. Not committed.

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

**Replacement-tracking field:** `Feature.replaces: Option<String>` (`#[serde(default, skip_serializing_if = "Option::is_none")]`). When the cascade replaced a placeholder (e.g. `ASI`) with this row (e.g. `Lucky`), `replaces` carries the placeholder's name. Authoritative source for `detect_replacement`'s fast path and for the swap-aware edit modal. `None` for direct adds and legacy rows.

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

### Locale data
- **Russian locale:** never put English placeholder text in `public/ru/` — the overlay system falls back to `en/` automatically. Omit untranslated entries.

### Git workflow
- One commit per logical task. Don't micro-commit.
- Commit messages: short title + at most one sentence on **why**. Don't list files, structs, function signatures, or per-piece changes — `git diff` shows that. The body explains intent (constraint, bug, user request); the diff explains content.
