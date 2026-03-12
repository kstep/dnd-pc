# Transient Effects Catalog

Unified list of all possible transient (temporary, toggled, activated) effects derived from spells and class/race/background features.

---

## 1. AC Bonus (Flat)
**Description:** Add a flat bonus to Armor Class.
**Conditions:** Active spell/feature, duration-limited.
**Formula:** `AC = AC + N`
**Sources:** Shield of Faith (+2), Warding Bond (+1), Ceremony/Wedding (+2), Agile Parry (+2), Arcane Deflection (+2), Soul of the Forge (+1)

## 2. AC Bonus (Ability)
**Description:** Add an ability modifier to Armor Class.
**Conditions:** Active feature, duration-limited.
**Formula:** `AC = AC + {ability}`
**Sources:** Bladesong (+INT)

## 3. AC Bonus (Die)
**Description:** Add a die roll to Armor Class.
**Conditions:** Resource expenditure (e.g. Bardic Inspiration).
**Formula:** `AC = AC + {die}`
**Sources:** Defensive Flourish (+BI die)

## 4. AC Override
**Description:** Set AC to a fixed formula, replacing normal calculation.
**Conditions:** Active spell, concentration or duration.
**Formula:** `AC = max(AC, N)` or `AC = N + {ability}`
**Sources:** Mage Armor (13 + DEX), Barkskin (max 17)

## 5. Temporary HP (Flat)
**Description:** Gain a fixed amount of temporary hit points.
**Conditions:** Activation (bonus action, rest, trigger event).
**Formula:** `TEMP_HP = N`
**Sources:** Fighting Spirit (5), Guardian of Nature/Tree (10)

## 6. Temporary HP (Level-Scaled)
**Description:** Gain temporary HP scaling with class level.
**Conditions:** Activation or trigger event.
**Formula:** `TEMP_HP = LEVEL + {ability}` or `TEMP_HP = LEVEL`
**Sources:** Vitality of the Tree (LEVEL), Dark One's Blessing (CHA + LEVEL), Celestial Resilience (LEVEL + CHA), Touch of Death (WIS + LEVEL), Armor of Agathys (5 * spell_level)

## 7. Temporary HP (Die + Ability)
**Description:** Gain temporary HP from a die roll plus ability modifier.
**Conditions:** Activation or trigger event.
**Formula:** `TEMP_HP = {die} + {ability}`
**Sources:** False Life (2d4+4), Tireless (1d8 + WIS), Bolstering Flames (1d4 + CHA), Improved Warding Flare (2d6 + WIS), Reclaim Potential (2d6 + CON), Form of Dread (1d10 + LEVEL), Hunter's Rime (1d10 + LEVEL)

## 8. Temporary HP (Proficiency)
**Description:** Gain temporary HP equal to proficiency bonus.
**Conditions:** Specific trigger (e.g. Dash action).
**Formula:** `TEMP_HP = proficiency_bonus`
**Sources:** Adrenaline Rush (Orc)

## 9. Max HP Bonus
**Description:** Temporarily increase maximum hit points (and current HP).
**Conditions:** Active spell, duration-limited.
**Formula:** `MAX_HP = MAX_HP + N`, `HP = HP + N`
**Sources:** Aid (+5, +5/slot above 2nd)

## 10. Speed Bonus (Flat)
**Description:** Increase walking speed by a fixed amount.
**Conditions:** Active spell/feature, duration-limited.
**Formula:** `SPEED = SPEED + N`
**Sources:** Longstrider (+10), Blade Flourish (+10), Drunken Technique (+10), Dread Ambusher (+10), Aura of Alacrity (+10), Guardian of Nature/Beast (+10), Ashardalon's Stride (+20), Zephyr Strike (+30), Large Form/Goliath (+10)

## 11. Speed Multiplier
**Description:** Multiply walking speed.
**Conditions:** Active spell, concentration.
**Formula:** `SPEED = SPEED * N`
**Sources:** Haste (*2)

## 12. Fly Speed (Equal to Walk)
**Description:** Gain a flying speed equal to walking speed.
**Conditions:** Active feature, duration-limited.
**Formula:** `FLY_SPEED = SPEED`
**Sources:** Steps of Night, Power of the Wilds/Falcon, Draconic Flight, Celestial Revelation/Wings

## 13. Fly Speed (Fixed)
**Description:** Gain a fixed flying speed.
**Conditions:** Active feature, duration-limited.
**Formula:** `FLY_SPEED = N`
**Sources:** Writhing Tide (10 ft)

## 14. Fly Speed (Multiplied)
**Description:** Gain a flying speed as a multiple of walking speed.
**Conditions:** Active feature, until end of turn.
**Formula:** `FLY_SPEED = SPEED * N`
**Sources:** Psi-Powered Leap (*2)

## 15. Attack Bonus (Flat)
**Description:** Add a flat bonus to attack rolls.
**Conditions:** Active spell/feature, duration-limited.
**Formula:** `ATTACK_BONUS += N`
**Sources:** Magic Weapon (+1/+2/+3)

## 16. Attack Bonus (Die)
**Description:** Add a die to attack rolls.
**Conditions:** Active spell, concentration.
**Formula:** `ATTACK_BONUS += {die}`
**Sources:** Bless (+1d4)

## 17. Damage Bonus (Flat)
**Description:** Add a flat bonus to damage rolls.
**Conditions:** Active spell/feature, on hit.
**Formula:** `DAMAGE_BONUS += N`
**Sources:** Magic Weapon (+1/+2/+3), Rage Damage (+2/+3/+4), Elemental Affinity (+CHA), Genie's Wrath (+proficiency_bonus), Celestial Revelation (+proficiency_bonus)

## 18. Damage Bonus (Die)
**Description:** Add extra damage dice on hit.
**Conditions:** Active spell/feature, once per turn or on specific attacks.
**Formula:** `DAMAGE_BONUS += {die}`
**Sources:** Divine Favor (+1d4), Hunter's Mark (+1d6), Elemental Weapon (+1d4), Crusader's Mantle (+1d4), Spirit Shroud (+1d8), Holy Weapon (+2d8), Dreadful Strikes (+1d4), Divine Strike (+1d8/+2d8), Primal Strike (+1d10), Sneak Attack ((LEVEL+1)/2 d6), Psychic Blades (+2d6), Defensive/Mobile/Slashing Flourish (+BI die), Divine Fury (+1d6 + LEVEL/2), Guardian of Nature/Beast (+1d6)

## 19. Damage Bonus (Smite, One-Shot)
**Description:** Add large burst damage dice on a single hit, consuming the spell.
**Conditions:** On hit, expends spell slot.
**Formula:** `DAMAGE_BONUS += {die}` (single attack)
**Sources:** Divine Smite (+2d8), Searing Smite (+1d6), Thunderous Smite (+2d6), Wrathful Smite (+1d6), Shining Smite (+2d6), Blinding Smite (+3d8), Staggering Smite (+4d6), Banishing Smite (+5d10)

## 20. Save Bonus (Flat)
**Description:** Add a flat bonus to saving throws.
**Conditions:** Active spell/feature, duration-limited.
**Formula:** `SAVE_BONUS += N`
**Sources:** Warding Bond (+1), Arcane Deflection (+4, single save)

## 21. Save Bonus (Die)
**Description:** Add a die to saving throws.
**Conditions:** Active spell, concentration.
**Formula:** `SAVE_BONUS += {die}`
**Sources:** Bless (+1d4), Ceremony/Dedication (+1d4)

## 22. Skill Bonus (Specific)
**Description:** Add a bonus to a specific skill check.
**Conditions:** Active spell, concentration or duration.
**Formula:** `{SKILL} += N`
**Sources:** Pass without Trace (Stealth +10)

## 23. Ability Check Bonus (Die)
**Description:** Add a die to ability checks.
**Conditions:** Active spell/feature, single use or duration.
**Formula:** `ABILITY_CHECK += {die}`
**Sources:** Guidance (+1d4), Ceremony/Coming of Age (+1d4)

## 24. Damage Resistance (Single Type)
**Description:** Gain resistance to a specific damage type (half damage).
**Conditions:** Active spell/feature, duration-limited or per-rest selection.
**Formula:** `RESISTANCE += {damage_type}`
**Sources:** Protection from Poison (Poison), Protection from Energy (chosen), Aura of Purity (Poison), Aura of Life (Necrotic), Absorb Elements (triggering type), Fiendish Resilience (chosen per rest)

## 25. Damage Resistance (Multiple Types)
**Description:** Gain resistance to multiple damage types simultaneously.
**Conditions:** Active feature/spell, duration-limited.
**Formula:** `RESISTANCE += {damage_type_1}, {damage_type_2}, ...`
**Sources:** Stoneskin (B/P/S), Full of Stars (B/P/S), Stormborn (Cold/Lightning/Thunder), Bear Totem (all except Psychic), Superior Defense (all except Force), Umbral Form (all except Force/Radiant)

## 26. Damage Reduction (Flat)
**Description:** Reduce incoming damage by a fixed amount.
**Conditions:** Reaction or active feature.
**Formula:** `DAMAGE_TAKEN -= N`
**Sources:** Song of Defense (5 * spell_slot_level)

## 27. Damage Reduction (Die)
**Description:** Reduce incoming damage by a die roll.
**Conditions:** Reaction, single use.
**Formula:** `DAMAGE_TAKEN -= {die}`
**Sources:** Resistance cantrip (1d4)

## 28. Advantage on Attacks
**Description:** Gain advantage on attack rolls.
**Conditions:** Active feature, duration-limited or single use.
**Formula:** `ADVANTAGE(ATTACK)` (boolean toggle)
**Sources:** Steady Aim (next attack, Speed=0), Entropic Ward (next attack after enemy miss), Guardian of Nature/Beast (STR attacks), Guardian of Nature/Tree (DEX/WIS attacks)

## 29. Advantage on Saves
**Description:** Gain advantage on saving throws (specific or all).
**Conditions:** Active feature/spell, duration-limited.
**Formula:** `ADVANTAGE(SAVE)` or `ADVANTAGE(SAVE_{ability})`
**Sources:** Guardian of Nature/Tree (CON saves), Enhance Ability (chosen ability checks), Circle of Power (vs. spells)

## 30. Advantage on Ability Checks
**Description:** Gain advantage on ability checks for a chosen ability.
**Conditions:** Active spell, concentration.
**Formula:** `ADVANTAGE(CHECK_{ability})`
**Sources:** Enhance Ability (chosen ability), Bladesong (Acrobatics)

## 31. Disadvantage on Enemy Attacks
**Description:** Impose disadvantage on enemy attack rolls against you.
**Conditions:** Active feature, duration-limited.
**Formula:** `DISADVANTAGE(ENEMY_ATTACK)` (boolean toggle)
**Sources:** Dispel Evil and Good (vs. specific creature types)

## 32. Speed Zero (Self-Imposed)
**Description:** Set own speed to 0 as cost of another benefit.
**Conditions:** Voluntary activation.
**Formula:** `SPEED = 0`
**Sources:** Steady Aim (until end of turn)

## 33. Condition Immunity
**Description:** Gain immunity to specific conditions.
**Conditions:** Active feature, while raging or transformed.
**Formula:** `IMMUNITY += {condition}`
**Sources:** Mindless Rage (Charmed/Frightened while raging), Form of Dread (Frightened while transformed)

---

## Summary by Attribute

| Attribute | Effect Count | Formula Complexity |
|-----------|-------------|-------------------|
| AC | 4 types (flat, ability, die, override) | Low-Medium |
| Temp HP | 4 types (flat, level-scaled, die+ability, proficiency) | Medium |
| Max HP | 1 type (flat bonus) | Low |
| Speed | 3 types (flat bonus, multiplier, zero) | Low |
| Fly Speed | 3 types (= walk, fixed, multiplied) | Low |
| Attack Bonus | 2 types (flat, die) | Low |
| Damage Bonus | 3 types (flat, die, smite one-shot) | Medium |
| Save Bonus | 2 types (flat, die) | Low |
| Skill Bonus | 1 type (specific skill) | Low |
| Ability Check | 1 type (die) | Low |
| Resistance | 2 types (single, multiple) | Categorical |
| Damage Reduction | 2 types (flat, die) | Low |
| Advantage/Disadvantage | 4 types (attack, save, check, enemy) | Boolean |
| Condition Immunity | 1 type | Categorical |

## Candidate Attribute Extensions for `Expr<Attribute>`

Currently supported: `AC`, `MAX_HP`, `HP`, `TEMP_HP`, `SPEED`, `LEVEL`, `CLASS_LEVEL`, `CASTER_LEVEL`, `Modifier(Ability)`

Needed for full coverage:
- `ATTACK_BONUS` — flat attack roll modifier
- `DAMAGE_BONUS` — flat damage modifier
- `SAVE_BONUS` — global saving throw modifier (or per-ability: `SAVE_STR`, etc.)
- `FLY_SPEED` — flying speed
- `SKILL_{name}` — per-skill bonus (or a general mechanism)
- Resistance/immunity/advantage are boolean/categorical, not numeric — need a separate mechanism (e.g. a set of active conditions/flags)
