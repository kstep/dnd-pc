# Review Leftovers

Findings from the BTreeMap refactoring review (2026-02-28) that were not addressed.
All are pre-existing issues or intentional design tradeoffs.

## Medium — Design

- [ ] `feature_data` is keyed by feature name (`BTreeMap<String, FeatureData>`).
  Cross-class features with the same name collide. Worked around by renaming features
  to include class name (e.g. "Spellcasting (Bard)"), but the proper fix is to key by
  `(class_name, feature_name)` tuple or similar composite key.
  Files: `src/model/character.rs` (`FeatureData`), `src/rules.rs` (`apply()`/`fill_descriptions()`)

## Medium — Code Quality

- [ ] `get_class()`/`get_race()`/`get_background()` clone entire definitions on every call.
  Add `with_class()`/`with_race()`/`with_background()` callback variants (like `with_feature()`).
  Files: `src/rules.rs` (lines ~717, ~912, ~958), callers in `features_panel.rs`, `character_header.rs`

- [ ] `fetch_class`/`fetch_race`/`fetch_background` — ~90 lines of near-identical code.
  Extract a generic `fetch_and_cache()` helper parameterized by cache signal and index field accessor.
  File: `src/rules.rs` (lines ~870–993)

- [ ] `serde_json::from_str(&format!("\"{val}\""))` used for enum parsing (Alignment, Ability).
  Add `FromStr` impls on enums instead.
  Files: `src/components/character_header.rs` (line ~364), `src/components/spellcasting_panel.rs` (line ~90)

## Low — Code Quality

- [ ] `with_class_entries`/`with_race_entries`/`with_background_entries` triplicated (~8 lines each).
  Unify with a generic helper or macro.
  File: `src/rules.rs` (lines ~708, ~903, ~949)

- [ ] `features_panel.rs` calls `registry.get_class()` (clones entire def) on every keystroke in
  the feature name input handler. Use `with_feature()` callback pattern instead.
  File: `src/components/features_panel.rs` (line ~72)

- [ ] `resolve_choice_options` is a static method on `RulesRegistry` but doesn't use `self`.
  Could be a free function or method on `FieldDefinition`.
  File: `src/rules.rs` (line ~855)

## Low — Clippy Pedantic/Nursery (pre-existing)

- [ ] `too_many_lines` — 6 functions exceed 100 lines (up to 406). Leptos component functions
  with `view!` macros; splitting requires extracting sub-components.
- [ ] `unnecessary_structure_name_repetition` — 82 instances. Use `Self::` instead of type name.
- [ ] `derive PartialEq → implement Eq` — 12 structs could also derive `Eq`.
- [ ] `must_use` — 10 getter methods could have `#[must_use]`.
- [ ] Casting warnings (`u32→i32`, `i32→u32`, etc.) — 8 instances across ability modifier
  and HP calculations. Values are small in practice (D&D range).

## Intentional / Won't Fix

- Name field stored redundantly in BTreeMap key + struct `.name` — acceptable tradeoff
- `named_map` silently drops duplicate names — data files have no duplicates
- BTreeMap alphabetical iteration order vs Vec JSON order — harmless for D&D features
- `SpellsDefinition.levels` and `ClassDefinition.levels` remain as `Vec` — contiguous level data
- `store.get()` broad tracking in Memos — computations are cheap, Leptos pattern
- `significant_drop_tightening` clippy warnings — false positives for Leptos RwSignal read guards
