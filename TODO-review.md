# Review Leftovers

Findings from the BTreeMap refactoring review (2026-02-28) that were not addressed.
All are pre-existing issues or intentional design tradeoffs.
Last checked against code: 2026-03-12.

## Medium — Code Quality

- [x] ~~`get_class()`/`get_race()`/`get_background()` clone entire definitions on every call.~~
  Fixed: refactored to `DefinitionStore` trait with `.with()` callback pattern in `src/rules/cache.rs`.

- [x] ~~`fetch_class`/`fetch_race`/`fetch_background` — ~90 lines of near-identical code.~~
  Fixed: deduplicated via generic `DefinitionStore` trait + `impl_definition_store!` macro.

- [x] ~~`serde_json::from_str` used for enum parsing (Alignment, Ability, DamageType).~~
  Fixed: added `TryFrom<u8>` in `enum_serde_u8!` macro, call sites use `value.parse::<u8>().ok().and_then(|n| T::try_from(n).ok())`.

## Low — Code Quality

- [x] ~~`with_class_entries`/`with_race_entries`/`with_background_entries`/`with_spell_entries` quadruplicated.~~
  Fixed: unified via `index_accessors!` macro in `src/rules/registry.rs`.

- [x] ~~`features_panel.rs` calls `registry.get_class()` (clones entire def) on every keystroke.~~
  Fixed: now uses `registry.classes().with(...)` zero-clone pattern in `src/components/panels/features.rs`.

- [x] ~~`resolve_choice_options` is a static method on `RulesRegistry` but doesn't use `self`.~~
  Fixed: moved to `FieldDefinition::resolve_choice_options()` in `src/rules/feature.rs`.

## Low — Clippy Pedantic/Nursery (pre-existing)

- [x] ~~`too_many_lines` — 6 functions exceed 100 lines.~~
  No longer flagged by clippy.
- [x] ~~`unnecessary_structure_name_repetition` — ~14 instances remain. Use `Self::` instead of type name.~~
  Fixed: replaced with `Self::` in all impl blocks in `enums.rs` and `rules/feature.rs`.
- [x] ~~`derive PartialEq → implement Eq` — 7 Params structs missing Eq.~~
  Fixed: added `Eq` derive to all 7 Params structs.
- [x] ~~Casting warnings (`u32→i32`, `i32→u32`, etc.)~~
  All casts are intentional and clippy-approved.

## Intentional / Won't Fix

- `must_use` on getters — not worth the noise

- `feature_data` keyed by feature name — features are unique across sources by design (e.g. "Spellcasting (Bard)")

- Name field stored redundantly in BTreeMap key + struct `.name` — acceptable tradeoff
- `named_map` silently drops duplicate names — data files have no duplicates
- BTreeMap alphabetical iteration order vs Vec JSON order — harmless for D&D features
- `SpellsDefinition.levels` and `ClassDefinition.levels` remain as `Vec` — contiguous level data
- `store.get()` broad tracking in Memos — computations are cheap, Leptos pattern
- `significant_drop_tightening` clippy warnings — false positives for Leptos RwSignal read guards
