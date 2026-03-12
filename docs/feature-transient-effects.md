# Feature Transient Effects Analysis

Investigation of all class, race, and background features for transient (temporary, activated, toggled) effects on character attributes. Excludes permanent/passive effects.

## AC Modifications (7 features)

| Feature | Source | Effect | Trigger/Duration |
|---------|--------|--------|-----------------|
| Defensive Flourish | Bard > College of Swords | `AC += Bardic Inspiration die` | Attack action, until start of next turn |
| Agile Parry | Monk > Way of the Kensei | `AC += 2` | While holding kensei weapon, until start of next turn |
| Arcane Deflection | Wizard > War Magic | `AC += 2` | Reaction, single attack |
| Song of Defense | Wizard > Bladesinging | `Damage -= 5 * spell_slot_level` | Reaction, while Bladesong active |
| Bladesong | Wizard > Bladesinging | `AC += INT` | Bonus action, 1 minute |
| Full of Stars | Druid > Circle of Stars | Resistance to B/P/S | While in Starry Form |
| Soul of the Forge | Cleric > Forge Domain | `AC += 1` | While wearing heavy armor |

## Temporary HP (23 features)

### Barbarian
| Feature | Source | Formula | Trigger |
|---------|--------|---------|---------|
| Reckless Abandon | Path of Battlerager | `TempHP = CON` | Reckless Attack while raging |
| Tundra Aura | Path of Storm Herald | `TempHP = 2` (scales) | Aura activation while raging |
| Vitality of the Tree | Path of World Tree | `TempHP = LEVEL` | Rage start + healing per turn |

### Bard
| Feature | Source | Formula | Trigger |
|---------|--------|---------|---------|
| Mantle of Inspiration | College of Glamour | `TempHP = 2 * Bardic Inspiration die` | Bonus action, expends BI |

### Cleric
| Feature | Source | Formula | Trigger |
|---------|--------|---------|---------|
| Improved Warding Flare | Light Domain | `TempHP = 2d6 + WIS` | Reaction |

### Fighter
| Feature | Source | Formula | Trigger |
|---------|--------|---------|---------|
| Fighting Spirit | Samurai | `TempHP = 5` | Bonus action |
| Reclaim Potential | Echo Knight | `TempHP = 2d6 + CON` | Echo destroyed |

### Monk
| Feature | Source | Formula | Trigger |
|---------|--------|---------|---------|
| Touch of Death | Way of Long Death | `TempHP = WIS + LEVEL` | Reduce creature to 0 HP |

### Ranger
| Feature | Source | Formula | Trigger |
|---------|--------|---------|---------|
| Tireless | Base class | `TempHP = 1d8 + WIS` | Magic action, limited uses |
| Hunter's Rime | Winter Walker | `TempHP = 1d10 + LEVEL` | Cast Hunter's Mark |

### Sorcerer
| Feature | Source | Formula | Trigger |
|---------|--------|---------|---------|
| Bolstering Flames | Spellfire Sorcery | `TempHP = 1d4 + CHA` | Bonus action |
| Honed Spellfire | Spellfire Sorcery | `TempHP bonus += LEVEL` | Enhancement |

### Warlock
| Feature | Source | Formula | Trigger |
|---------|--------|---------|---------|
| Dark One's Blessing | The Fiend | `TempHP = CHA + LEVEL` | Reduce enemy to 0 HP |
| Form of Dread | The Undead | `TempHP = 1d10 + LEVEL` | Transform (bonus action, 1 min) |
| Celestial Resilience | The Celestial | `TempHP = LEVEL + CHA` | Short/Long Rest |

### Racial
| Feature | Source | Formula | Trigger |
|---------|--------|---------|---------|
| Adrenaline Rush | Orc | `TempHP = proficiency_bonus` | Dash as bonus action |

## Speed Modifications (20+ features)

### Temporary Activation
| Feature | Source | Formula | Trigger/Duration |
|---------|--------|---------|-----------------|
| Blade Flourish | Bard > College of Swords | `Speed += 10` | Attack action, until end of turn |
| Drunken Technique | Monk > Way of Drunken Master | `Speed += 10` | Flurry of Blows, until end of turn |
| Psi-Powered Leap | Fighter > Psi Warrior | `FlySpeed = Speed * 2` | Bonus action, until end of turn |
| Dread Ambusher | Ranger > Gloom Stalker | `Speed += 10` | First turn of combat only |
| Writhing Tide | Ranger > Swarmkeeper | `FlySpeed = 10` | Bonus action, 1 minute |
| Steps of Night | Cleric > Twilight Domain | `FlySpeed = Speed` | Bonus action, 1 minute |
| Power of the Wilds (Falcon) | Barbarian > Path of Wild Heart | `FlySpeed = Speed` | While raging |
| Aura of Alacrity | Paladin > Oath of Glory | `Speed += 10` (allies in aura) | Aura, until end of next turn |

### Racial
| Feature | Source | Formula | Trigger/Duration |
|---------|--------|---------|-----------------|
| Draconic Flight | Dragonborn | `FlySpeed = Speed` | Bonus action, 10 min, 1/Long Rest |
| Large Form | Goliath | `Speed += 10` | Bonus action, 10 min, 1/Long Rest |
| Celestial Revelation (Wings) | Aasimar | `FlySpeed = Speed` | 1 minute, 1/Long Rest |

## Attack / Damage Bonuses (35+ features)

### Barbarian (Rage-Dependent)
| Feature | Source | Formula | Trigger |
|---------|--------|---------|---------|
| Rage Damage | Base class | `Damage += 2/3/4` (by level) | STR attacks while raging |
| Frenzy | Path of Berserker | `Damage += rage_damage * d6` | First STR attack/turn while raging+reckless |
| Divine Fury | Path of Zealot | `Damage += 1d6 + LEVEL/2` | First hit/turn while raging |
| Primal Strike | Base class (11) | `Damage += 1d10` | Weapon attacks while raging |

### Bard (Bardic Inspiration)
| Feature | Source | Formula | Trigger |
|---------|--------|---------|---------|
| Defensive Flourish | College of Swords | `Damage += BI_die` | Expends Bardic Inspiration |
| Mobile Flourish | College of Swords | `Damage += BI_die` | Expends Bardic Inspiration |
| Slashing Flourish | College of Swords | `Damage += BI_die` | Expends Bardic Inspiration, multi-target |
| Psychic Blades | College of Whispers | `Damage += 2d6 Psychic` | Once/turn, expends BI |

### Cleric
| Feature | Source | Formula | Trigger |
|---------|--------|---------|---------|
| Divine Strike | Various domains | `Damage += 1d8` (→ 2d8 at 14) | Once/turn on weapon hit |

### Ranger
| Feature | Source | Formula | Trigger |
|---------|--------|---------|---------|
| Dreadful Strikes | Fey Wanderer | `Damage += 1d4 Psychic` | Once/turn on weapon hit |

### Rogue
| Feature | Source | Formula | Trigger |
|---------|--------|---------|---------|
| Sneak Attack | Base class | `Damage += (LEVEL+1)/2 d6` | Once/turn with advantage or ally adjacent |

### Sorcerer
| Feature | Source | Formula | Trigger |
|---------|--------|---------|---------|
| Elemental Affinity | Draconic Sorcery | `Damage += CHA` | Matching element spells |

### Warlock
| Feature | Source | Formula | Trigger |
|---------|--------|---------|---------|
| Genie's Wrath | The Genie | `Damage += proficiency_bonus` | Once/turn on hit |

### Racial
| Feature | Source | Formula | Trigger |
|---------|--------|---------|---------|
| Celestial Revelation | Aasimar | `Damage += proficiency_bonus Radiant` | Once/turn, 1 min, 1/Long Rest |

## Saving Throw Bonuses (8 features)

| Feature | Source | Formula | Trigger |
|---------|--------|---------|---------|
| Arcane Deflection | Wizard > War Magic | `Save += 4` | Reaction, single save |
| Disciplined Survivor | Monk | Reroll failed save | Expends Focus Point |
| Restore Balance | Sorcerer > Clockwork Soul | Prevent advantage/disadvantage | Reaction |
| Trance of Order | Sorcerer > Clockwork Sorcery | Prevent advantage/disadvantage | Bonus action, 1 minute |
| Staggering Blow | Barbarian | Enemy disadvantage on next save | Until start of next turn |
| Ceremony (Dedication) | Paladin spell | `Save += 1d4` | 24 hours |
| Warding Bond | Cleric/Paladin spell | `Save += 1` | Concentration |
| Bless | Cleric/Paladin spell | `Save += 1d4` | Concentration |

## Resistance / Immunity (Transient Only)

### Rage-Based
| Feature | Source | Effect | Duration |
|---------|--------|--------|----------|
| Bear Totem | Barbarian > Totem Warrior | Resistance to all except Psychic | While raging |
| Mindless Rage | Path of Berserker | Immunity to Charmed/Frightened | While raging |

### Form/Transformation-Based
| Feature | Source | Effect | Duration |
|---------|--------|--------|----------|
| Full of Stars | Druid > Circle of Stars | Resistance to B/P/S | While in Starry Form |
| Stormborn | Druid > Circle of Sea | Resistance Cold/Lightning/Thunder | While Wrath of Sea active |
| Superior Defense | Monk | Resistance to all except Force | 1 minute, 3 Focus Points |
| Umbral Form | Sorcerer > Shadow Magic | Resistance to all except Force/Radiant | 6 Sorcery Points |

### Reaction-Based
| Feature | Source | Effect | Duration |
|---------|--------|--------|----------|
| Dampen Elements | Cleric > Nature Domain | Resistance to acid/cold/fire/lightning/thunder | Single damage instance |

### Per-Rest Selection
| Feature | Source | Effect | Duration |
|---------|--------|--------|----------|
| Fiendish Resilience | Warlock > The Fiend | Resistance to chosen type | Until next Short/Long Rest |

## Other Transient Effects

### Advantage/Disadvantage
| Feature | Source | Effect | Trigger |
|---------|--------|--------|---------|
| Steady Aim | Rogue | Advantage on next attack, `Speed = 0` | Bonus action |
| Tides of Chaos | Sorcerer > Wild Magic | Advantage on one roll | 1/Long Rest |
| Entropic Ward | Warlock > Great Old One | Disadvantage on enemy attack, advantage on your next attack if miss | Reaction |

### Transformations (Multiple Attribute Changes)
| Feature | Source | Effects | Duration |
|---------|--------|---------|----------|
| Bladesong | Wizard > Bladesinging | `AC += INT`, `Speed += 10`, advantage on Acrobatics | Bonus action, 1 minute |
| Form of Dread | Warlock > The Undead | `TempHP = 1d10 + LEVEL`, immunity to Frightened | Bonus action, 1 minute |
| Guardian of Nature (Beast) | Ranger | `Speed += 10`, advantage on STR attacks, `Damage += 1d6` | 1 minute |
| Guardian of Nature (Tree) | Ranger | `TempHP += 10`, advantage on CON saves, advantage on DEX/WIS attacks | 1 minute |

## Design Patterns

### Activation Costs
- **Bonus action** — most common (Bladesong, Fighting Spirit, Form of Dread)
- **Reaction** — defensive triggers (Arcane Deflection, Dampen Elements)
- **Resource expenditure** — Bardic Inspiration, Focus Points, Sorcery Points, Rage uses
- **Automatic** — on kill (Dark One's Blessing), on hit (Divine Strike), while raging

### Duration Types
- Until start/end of next turn (shortest)
- 1 minute (≈10 rounds, most combat features)
- 10 minutes (racial transformations)
- Until rage/form ends (state-dependent)
- Until next Short/Long Rest (selection-based)

### Formula Components
- Ability modifiers: `STR`, `DEX`, `CON`, `INT`, `WIS`, `CHA`
- Class level: `LEVEL` or `LEVEL/2`
- Proficiency bonus: `proficiency_bonus`
- Dice: `d4`, `d6`, `d8`, `d10`, `d12` (often scaling)
- Fixed values: `+1`, `+2`, `+5`, `+10`
