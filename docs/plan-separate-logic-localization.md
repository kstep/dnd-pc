# Plan: Separate Logic from Localization

## Goal

Split each locale-specific data file (classes, races, backgrounds, spells, effects, index) into two parts:
1. **Data file** (locale-independent) — logic, numbers, mechanics (`public/data/`)
2. **Locale file** (locale-specific) — labels, descriptions, short names (`public/{locale}/`)

The locale files use a **flat key→text map** format instead of mirroring the deep data structure.

## Current State

Each locale has its own full copy of every file:
- `public/{en,ru}/classes/*.json` — full class definitions with labels+descriptions embedded
- `public/{en,ru}/races/*.json`, `backgrounds/*.json`, `spells/*.json` — same pattern
- `public/{en,ru}/index.json`, `effects.json`

The `ru/` files duplicate all numeric/structural data from `en/` and add `label` fields + translated `description` fields. This means every mechanical change must be made in both locales.

## Target Layout

```
public/
  data/
    classes/wizard.json       # mechanics only: hit_die, features (name, spells config, fields), levels, subclasses
    races/tiefling.json       # mechanics only: traits (name), features, speed, abilities
    backgrounds/acolyte.json  # mechanics only: features, abilities, skills
    spells/wizard.json        # mechanics only: name, level, sticky, min_level, cost
    index.json                # names + urls only (no descriptions)
    effects.json              # name + expr only (no descriptions)
  en/
    classes/wizard.json       # flat locale map
    races/tiefling.json       # flat locale map
    backgrounds/acolyte.json  # flat locale map
    spells/wizard.json        # flat locale map
    index.json                # flat locale map
    effects.json              # flat locale map
  ru/
    ... (same structure)
```

## Flat Locale Map Format

Each locale file is a `BTreeMap<LocaleKey, LocaleText>`:

```json
{
  "description": "A scholarly magic-user...",
  "feature.Arcane Recovery": { "label": "Arcane Recovery", "description": "You can regain..." },
  "feature.Spellcasting (Wizard)": { "label": "Spellcasting (Wizard)", "description": "As a student..." },
  "subclass.School of Evocation": { "label": "School of Evocation", "description": "You focus..." },
  "subclass.School of Evocation.feature.Evocation Savant": { "label": "...", "description": "..." },
  "feature.Metamagic.field.Metamagic Options": { "label": "...", "description": "..." },
  "feature.Metamagic.field.Metamagic Options.option.Careful Spell": { "label": "...", "description": "..." },
  "feature.Spellcasting (Wizard).spell.Fireball": { "label": "...", "description": "..." }
}

// For en/ locale, labels that match the name can be omitted (they're the same)
// For ru/ locale, labels are the translated display names
```

The top-level `"description"` (no dot) is the entity's own description (class/race/background).

### Key grammar

```
<root>       ::= "label" | "description"
<feature>    ::= "feature." <name> ("." <prop>)?
<subclass>   ::= "subclass." <name> ("." <prop> | ".feature." <name> ("." <prop>)?)
<prop>       ::= "label" | "description"
               | "field." <name> ("." <prop> | ".option." <name> ("." <prop>)?)
               | "spell." <name> ("." <prop>)?
<trait>      ::= "trait." <name> ("." <prop>)?     # for races
```

When a key like `"feature.X"` has no trailing `.label`/`.description`, its value is a `LocaleText` object `{ "label": "...", "description": "..." }`. Both fields are optional.

### Spell list locale files

```json
// en/spells/wizard.json
{
  "Acid Splash": { "description": "You create an acidic bubble..." },
  "Fireball": { "label": "Fireball", "description": "A bright streak..." }
}
```

Simple flat `name → { label?, description? }` — no path prefixes needed since spells are a flat list.

### Index locale file

```json
// en/index.json
{
  "class.Wizard": { "description": "A scholarly magic-user..." },
  "class.Artificer": { "description": "A master of invention..." },
  "race.Tiefling": { "description": "Fiendish heritage..." },
  "background.Acolyte": { "description": "Temple servant..." }
}
```

### Effects locale file

```json
// en/effects.json
{
  "Shield of Faith": { "description": "+2 AC (concentration, 10 min)" },
  "Bladesong": { "description": "AC += INT.MOD; SPEED += 10" }
}
```

## Rust Types

### New types (`src/rules/locale.rs`)

```rust
/// A dot-separated key in a locale map, e.g. "feature.Arcane Recovery"
/// or "subclass.School of Evocation.feature.Evocation Savant"
struct LocaleKey(Box<str>);

/// Parsed path from a LocaleKey
enum LocalePath<'a> {
    Root,                                              // "" (top-level label/description)
    Feature(&'a str),                                  // "feature.X"
    FeatureField(&'a str, &'a str),                    // "feature.X.field.Y"
    FeatureFieldOption(&'a str, &'a str, &'a str),     // "feature.X.field.Y.option.Z"
    FeatureSpell(&'a str, &'a str),                    // "feature.X.spell.Y"
    Subclass(&'a str),                                 // "subclass.X"
    SubclassFeature(&'a str, &'a str),                 // "subclass.X.feature.Y"
    SubclassFeatureField(&'a str, &'a str, &'a str),   // "subclass.X.feature.Y.field.Z"
    // ...extend as needed
    Trait(&'a str),                                    // "trait.X" (races)
    Unknown,
}

/// The value for every locale entry
struct LocaleText {
    label: Option<String>,
    description: Option<String>,
    short: Option<String>,
}

/// A complete locale map for one entity (one file)
type LocaleMap = BTreeMap<LocaleKey, LocaleText>;
```

`LocaleKey` implements `Deserialize` (from string), `Ord`, `Eq`, `Hash`, and has a `parse() -> LocalePath` method that splits by `.` and matches the known grammar.

### Data file changes

Data structs (`ClassDefinition`, `RaceDefinition`, etc.) lose their `label` and `description` fields. Or rather — those fields become `#[serde(skip)]` on the data struct and are populated from the locale map after loading.

**Alternative (simpler):** Keep `label`/`description` fields on data types, but the data JSON files simply don't include them (they `#[serde(default)]` already). The locale map is applied after deserialization to fill them in. This avoids changing any Rust struct definitions — only the JSON files change and the loading code.

→ **Go with the simpler alternative.** The structs keep their fields, the data files just don't have them.

## Implementation Steps

### Phase 1: New types and locale loading

1. Create `src/rules/locale.rs` with `LocaleKey`, `LocalePath`, `LocaleText`, `LocaleMap`
2. Add `LocaleKey::parse() -> LocalePath` with the segment-matching logic
3. Add `LocaleText::apply_label(&self, target: &mut Option<String>)` and `apply_description(&self, target: &mut String)` helpers
4. Add deserialization for `LocaleMap` (custom `Deserialize` for `LocaleKey` from string, standard for `LocaleText` — except top-level `"description"` and `"label"` keys which are bare strings, not objects... actually, keep it uniform: the root entity uses key `"description"` with value `{"description": "..."}` is redundant. Better: top-level keys are just `"label"` and `"description"` as bare strings → needs special handling in deserialization OR use a different root key convention)

Actually, simplest: the root entry uses an **empty key** `""`:
```json
{
  "": { "label": "Волшебник", "description": "Учёный маг..." },
  "feature.Arcane Recovery": { "label": "...", "description": "..." }
}
```
Then every value is uniformly `LocaleText`. Clean.

### Phase 2: Locale application to definitions

5. Add `apply_locale(def: &mut ClassDefinition, locale: &LocaleMap)` — iterates locale map, matches `LocalePath`, fills labels/descriptions on the definition struct
6. Same for `RaceDefinition`, `BackgroundDefinition` (or a trait if patterns converge)
7. For spell lists: `apply_spell_locale(spells: &mut SpellMap, locale: &LocaleMap)` — simpler since it's flat name→text

### Phase 3: Split data files

8. Create `public/data/` with mechanics-only JSON (strip all label/description from current `en/` files)
9. Create flat locale maps in `public/en/` and `public/ru/` from the stripped content
10. Write a one-time script to do the conversion automatically

### Phase 4: Update loading pipeline

11. Modify `FetchCache` / `DefinitionStore` to load from `public/data/` (locale-independent URL) + `public/{locale}/` (locale-specific URL)
12. After both arrive, apply locale to definition before caching
13. On locale change: re-fetch only locale files, re-apply to cached definitions
14. Update `index.html` Trunk copy-dir to include `public/data/`

### Phase 5: Update `sync_labels` / `fill_from_registry` / `clear_from_registry`

15. `fill_from_registry` already works against the cached definitions which now have labels filled from locale → no change needed (labels come from locale→definition→character, same pipeline)
16. `clear_from_registry` similarly works against cached definitions → no change needed
17. Verify the locale-change flow: clear caches → re-fetch → re-fill → triggers character label refresh

### Phase 6: Update sharing pipeline

18. `strip_for_sharing` / `clear_from_registry` should still work since definitions have labels populated from locale
19. Verify compressed URL sharing still works (labels stripped, re-filled on import)

### Phase 7: Spell list and effects/index locale

20. Split `spells/*.json` into `data/spells/` (name, level, sticky, min_level, cost) + `{locale}/spells/` (flat name→text)
21. Split `index.json` into `data/index.json` (names, urls, prerequisites) + `{locale}/index.json` (descriptions)
22. Split `effects.json` into `data/effects.json` (name, expr) + `{locale}/effects.json` (descriptions)

## Open Questions

1. **Locale cache invalidation:** When locale changes, do we re-fetch data files too, or just locale files? → Just locale files. Data is locale-independent, cached once.
2. **Fallback:** If a locale file is missing (e.g. new class added to data but not yet translated), the definition works fine with empty labels — `fill_from_registry` already handles `None` labels gracefully.
3. **Script for conversion:** A Python script that reads current `en/*.json` and `ru/*.json`, splits into `data/` + flat locale maps. Run once, delete after.
