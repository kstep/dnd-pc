# Spell Catalog Migration — Conflict Resolutions

When unifying nine per-class JSON files into a single `public/data/spells.json`,
the migration script (`scripts/migrate_spells.py`) flagged divergences in
spells that appeared in multiple class lists with different *functional* fields
(`level` / `ritual` / `concentration` / `cast_time` / `effects`).

Each conflict was cross-checked against [dnd2024.wikidot.com](http://dnd2024.wikidot.com/)
and resolved by picking the canonical version. This document records each
decision so future audits can verify the catalog matches the 2024 PHB.

## A. Per-class catalog conflicts (21 → 1 canonical each)

All 21 conflicts were in the `effects` field. Categories:

### A.1 Effect-name harmonisation (no mechanic change)

| Spell | Old per-class names | Canonical | Notes |
|---|---|---|---|
| Bless | "Bonus" (cleric) vs "Bless" (paladin) | `name: "Bless"`, `expr: "1d4"` | Use spell name |
| Bane | "Penalty" (bard) vs "Bane" (warlock) | `name: "Bane"`, `expr: "1d4"` | Use spell name |
| Enlarge/Reduce | "Enlarge/Reduce" (artificer) vs "Damage Bonus" (bard/druid/sorcerer/wizard) | `name: "Damage Bonus"`, `expr: "1d4"` | Majority view + describes the mechanic |
| Fire Shield | "Fire Shield" (druid) vs "Fire/Cold Damage" (sorcerer/wizard) | `name: "Fire/Cold Damage"`, `expr: "2d8"` | Type-accurate |
| Elminster's Effulgent Spheres | "Damage" (druid) vs "Force Damage" (wizard) | `name: "Force Damage"`, `expr: "4d8"` | Damage type explicit |

### A.2 Mechanic corrections (wiki-driven)

| Spell | Loser variant | Canonical | Wiki ground truth |
|---|---|---|---|
| Insect Plague | sorcerer `4d10` flat | cleric `(SLOT_LEVEL - 1)d10` | "4d10 piercing damage at 5th level. +1d10 per slot above 5" → formula gives 4 at SL 5, scales correctly |
| Elemental Bane | druid/warlock/wizard had **no** expr | artificer `"Damage Bonus" 2d6` | Spell adds 2d6 of chosen type to subsequent attacks |
| Circle of Power | wizard had **no** expr | artificer `"STR.SAVE.ADV = 1; …; CHA.SAVE.ADV = 1"` | Grants advantage on all six saves vs magic |
| Jallarzi's Storm of Radiance | wizard had **two** effects (Radiant + Thunder) | warlock single `"Radiant Damage" 2d10` | Spell deals only Radiant damage |
| False Life | artificer raw `2d4 + 4 + 5*(SLOT_LEVEL-1)` | sorcerer/wizard `TEMP_HP = max(TEMP_HP, …)` | Temp HP cannot stack — must take max |
| Thunderwave | sorcerer/wizard `((SLOT_LEVEL-1)*3)d8` (gives 0 at SL 1!) | bard `(SLOT_LEVEL + 1)d8` | 2d8 base + 1d8 per upcast |
| Ashardalon's Stride | artificer **only** Speed effect | sorcerer/wizard Speed + `"Fire Damage" (SLOT_LEVEL-2)d6` | Spell triggers fire damage when entering 5 ft of a creature |

## B. Inline-vs-catalog conflicts (50 → 4 catalog fixes, 46 inline-discarded)

The 50 inline `effects` overrides in `features.json` (subclass spell lists like
`Alchemist Spells`, `Life Domain Spells`, etc.) were compared with the
canonical catalog. In all but four cases, the catalog held the correct value
and the inline override was simply stale.

### B.1 Catalog updated to match inline (wiki-confirmed)

| Spell | Catalog had | Updated to | Wiki ground truth |
|---|---|---|---|
| Magic Weapon | `ATK += if(SLOT_LEVEL >= 6, 3, if(SLOT_LEVEL >= 3, 2, 1))` (gave +2 at SL 3) | `ATK += SLOT_LEVEL / 2` | Wiki: +1 base, +2 at slot 4, +3 at slot 6 — `SLOT_LEVEL / 2` is correct |
| Spiritual Weapon | `1d8 + CASTER_MODIFIER` (no upcast scaling) | `(SLOT_LEVEL / 2)d8 + CASTER_MODIFIER` | Wiki: 1d8 base, +1d8 per 2 slot levels above 2 |

### B.1a Inline corrections that were *wrong* (catalog kept)

The inline copies of Fear and Lightning Bolt used `range: "Caster"`, but the
project convention reserves `"Caster"`/`"Self"` for spells whose caster *is*
the target. For area spells like cones and lines, `range` records how far the
effect reaches — i.e. `Feet: N`. Catalog values of `Feet: 30` (Fear) and
`Feet: 100` (Lightning Bolt) are correct; inline `"Caster"` overrides were
discarded along with the rest of B.2.

### B.2 Inline overrides discarded as stale (46 cases)

These features had inline `effects` arrays diverging from the catalog. After
cross-checking with the wiki, the catalog was correct in every case — the
inline values were old or wrong. Per the new schema, `SpellEntry` only carries
`{name, sticky?, min_level?, cost?}`, so functional fields are inherited from
the catalog automatically.

Affected spells (across various subclass spell lists like Alchemist, Armorer,
Artillerist, Battle Smith, Life Domain, Light Domain, War Domain, Circle of the
Land, Circle of the Moon, Circle of the Sea, Oath of Devotion, Oath of the
Ancients, Oath of Vengeance, Oath of Glory, Genie, Gloom Stalker, Winter
Walker, Psionic, Clockwork, Draconic, Spellfire, Celestial, Fiend, Great Old
One, Chthonic Legacy):

- Mass Healing Word
- Vitriolic Sphere
- Cloudkill
- Lightning Bolt (per-feature copies — already covered by catalog fix in B.1)
- Shield (×2 features)
- Thunderwave (already in A.2)
- Ice Storm (×5 features)
- Warding Bond
- Conjure Barrage
- Mass Cure Wounds (×3)
- Bless (×1 feature — catalog now canonical from A.1)
- Cure Wounds (×4)
- Aid (×4)
- Burning Hands (×3)
- Flame Strike (×3)
- Magic Weapon (already in B.1)
- Shield of Faith (×2)
- Spiritual Weapon (already in B.1)
- Bane (already in A.1)
- Haste (×2)
- Fear (already in B.1)
- Hunger of Hadar (×2)
- Fire Shield / Ice Knife / Thunderous Smite (single feature each)
- Chill Touch / False Life (Chthonic Legacy)

## C. Locale text divergences (non-fatal)

The per-locale spell files (`public/{en,ru}/spells/*.json`) had similar
duplication: 49 description divergences in `en/`, 740 in `ru/`. Spot-checks
showed these were trivial wording artefacts ("Action (ritual)" vs "Ritual,
action"; "made of cloud" vs "made of clouds"; "itselfto" vs "itself to") —
not worth manual review for ~800 cases. Policy: alphabetical first-wins per
locale (artificer < bard < cleric < … < wizard). The merged overlay lives at
`public/{en,ru}/spells.json`. Use `git diff` against the pre-migration tree
to audit specific spells if a regression surfaces.

## Migration script

`scripts/migrate_spells.py` is a one-shot script. It exits with code 1 on any
unresolved functional conflict in the per-class catalog (forcing manual fix-up
before re-run) and warns on locale divergences. It is removed in the cleanup
commit at the end of the spells-catalog branch.
