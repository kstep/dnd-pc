# Spell Attribute Effects Analysis

Investigation of all spells in `public/en/spells/` for effects expressible as formulas on character attributes.

## AC Modifications

| Spell | Level | Lists | Formula |
|-------|-------|-------|---------|
| Mage Armor | 1 | Sorcerer, Wizard | `AC = 13 + DEX` |
| Shield of Faith | 1 | Paladin, Cleric | `AC += 2` |
| Barkskin | 2 | Ranger | `AC = max(AC, 17)` |
| Warding Bond | 2 | Paladin, Cleric | `AC += 1` |
| Ceremony (Wedding) | 1 | Paladin | `AC += 2` (conditional) |

## HP / Temporary HP

| Spell | Level | Lists | Formula |
|-------|-------|-------|---------|
| False Life | 1 | Sorcerer, Wizard | `TempHP = 2d4+4` (+5/slot above 1st) |
| Heroism | 1 | Paladin, Cleric | `TempHP += ability_mod` per turn |
| Armor of Agathys | 1 | Warlock | `TempHP = 5 * spell_level` |
| Aid | 2 | Ranger, Paladin, Cleric | `MAX_HP += 5, HP += 5` (+5/slot above 2nd) |
| Guardian of Nature (Tree) | 4 | Ranger | `TempHP += 10` |

## Speed

| Spell | Level | Lists | Formula |
|-------|-------|-------|---------|
| Longstrider | 1 | Ranger | `Speed += 10` |
| Zephyr Strike | 1 | Ranger | `Speed += 30` (one turn) |
| Ashardalon's Stride | 3 | Ranger | `Speed += 20` (+5/slot above 3rd) |
| Guardian of Nature (Beast) | 4 | Ranger | `Speed += 10` |
| Haste | 3 | Sorcerer, Wizard | `Speed *= 2` |

## Attack / Damage Bonuses

| Spell | Level | Lists | Formula |
|-------|-------|-------|---------|
| Bless | 1 | Cleric, Paladin | `Attack += 1d4, Save += 1d4` |
| Divine Favor | 1 | Paladin, Cleric | `Damage += 1d4 Radiant` |
| Hunter's Mark | 1 | Ranger | `Damage += 1d6 Force` |
| Magic Weapon | 2 | Many | `Attack += 1/2/3, Damage += 1/2/3` (by slot) |
| Elemental Weapon | 3 | Ranger, Paladin, Druid | `Attack += 1, Damage += 1d4` |
| Crusader's Mantle | 3 | Paladin, Cleric | `Damage += 1d4 Radiant` (aura) |
| Spirit Shroud | 3 | Paladin, Cleric | `Damage += 1d8` (+1d8/2 slots) |
| Holy Weapon | 5 | Paladin | `Damage += 2d8 Radiant` |
| Smite spells | 1-5 | Paladin | Various: `+1d6` to `+5d10` |

## Saving Throw / Skill Bonuses

| Spell | Level | Lists | Formula |
|-------|-------|-------|---------|
| Guidance | 0 | Cleric, Druid, Bard | `Ability Check += 1d4` |
| Resistance | 0 | Cleric | `Damage taken -= 1d4` |
| Bless | 1 | Cleric, Paladin | `Save += 1d4` |
| Warding Bond | 2 | Paladin, Cleric | `Save += 1` |
| Pass without Trace | 2 | Ranger | `Stealth += 10` |
| Ceremony (Coming of Age) | 1 | Paladin | `Ability Check += 1d4` |
| Ceremony (Dedication) | 1 | Paladin | `Save += 1d4` |

## Damage Resistance

| Spell | Level | Lists | Formula |
|-------|-------|-------|---------|
| Absorb Elements | 1 | Ranger, Sorcerer, Wizard | Resistance to triggering type |
| Protection from Poison | 2 | Ranger, Cleric | Resistance: Poison |
| Protection from Energy | 3 | Ranger, Druid | Resistance: 1 chosen type |
| Stoneskin | 4 | Ranger | Resistance: Bludgeoning/Piercing/Slashing |
| Aura of Purity | 4 | Paladin | Resistance: Poison |
| Aura of Life | 4 | Paladin | Resistance: Necrotic |

## Mapping to Current `Expr<Attribute>` System

### Directly expressible with existing `Attribute` variants:
- `AC += N` → `AC = AC + N`
- `Speed += N` → `SPEED = SPEED + N`
- `MAX_HP += N` → `MAX_HP = MAX_HP + N`
- `HP += N` → `HP = HP + N`
- `TempHP = N` → `TEMP_HP = N`

### Would need new `Attribute` variants or extensions:
- **Save bonuses** (per-ability or global) — no `SaveBonus` attribute yet
- **Attack/Damage bonuses** — no `AttackBonus`/`DamageBonus` attribute yet
- **Skill-specific bonuses** (e.g. Stealth +10) — no per-skill attribute
- **Damage resistance** — categorical, not numeric (needs different mechanism)
- **Dice-based bonuses** (`+1d4`) — `Expr` supports dice notation but semantics are tricky (roll once vs. per-use)
- **Conditional effects** (advantage, per-turn, on-hit) — not formula-expressible

### Best candidates for implementation (clean numeric formulas):
Mage Armor, Shield of Faith, Longstrider, Aid, Bless, Warding Bond, Magic Weapon, Pass without Trace
