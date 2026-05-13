## Page-level
page-characters = D&D 5e Characters
btn-new-character = + New Character
btn-load-character = Load from File
page-not-found = Page not found
character-not-found = Character not found
back-to-list = Back to character list
btn-delete = Delete
btn-cancel = Cancel
confirm-delete = Delete this character?

## Character header
character-name = Character Name
species = Species
background = Background
alignment = Alignment
xp = XP
total-level = Total Level
prof-bonus = Prof. Bonus
classes = Classes
class = Class
subclass = Subclass
btn-add-class = + Add / Level Up
btn-edit-feature = Edit feature inputs
apply = Apply
apply-features-title = Apply features
build-replay-hint-title = These features were edited or not yet applied. Click Rebuild to re-apply.
build-choice-hint-title = These features need a choice selected:
build-pending-apply-title = New levels or changes have not been applied yet. Click Apply to add related features.
build-needs-rebuild-title = Character {$reasons}. Rebuild required.
rebuild-reason-species = species changed
rebuild-reason-background = background changed
rebuild-reason-class-removed = class "{$class}" removed
rebuild-reason-level-lowered = class "{$class}" level lowered (was {$applied}, now {$current})
rebuild-reason-subclass-changed = subclass changed for class "{$class}"
rebuild-reason-feature-removed = feature "{$name}" removed
rebuild-reason-legacy-system-markers = legacy data format
replace-with-feat = Replace with…
no-eligible-options = No eligible options available
export-json = Save to file
import-json = Load from file
reset-character = Reset character
actions-menu = Actions
share-link = Share link
share-toggle = Public sharing
share-loading = Loading shared character...
share-not-found = Character not found or not shared
hint-no-characters = Don't see your characters? Without signing in to the cloud they exist only on this device and may be lost when browser data is cleared. Sign in with Google to sync across devices.
hint-character-not-found = Is this your character on a different account? Sign in with Google to access it.
hint-sign-in-button = Sign in with Google
toast-signin-prompt = Your characters live only on this device and may be lost when browser data is cleared.
toast-signin-action = Sign in
toast-dismiss = Dismiss
toast-export-copied = Character JSON copied to clipboard. Telegram Mini Apps can't save files directly — paste it into Saved Messages (or any chat) to keep the data.
toast-export-copy-failed = Couldn't copy character JSON to the clipboard.
toast-sync-error = Sync error: { $error }
toast-login-tma = Google sign-in doesn't work inside Telegram. The link has been copied — paste it in a browser to sign in.
update-available = A new version is available.
update-button-reload = Update
copy-character = Copy character
import-conflict-title = Character already exists
import-conflict-message = You already have a newer version of "{$name}". Importing will overwrite it. See differences below.
import-anyway = Import Anyway
import-as-copy = Import as Copy
import-cancel = Cancel
diff-section-identity = Identity
diff-field = Field
diff-local = Local
diff-imported = Imported
diff-no-differences = No visible differences
no-class = No class
level-prefix = Level

## Panel titles
panel-ability-scores = Ability Scores
panel-saving-throws = Saving Throws
panel-skills = Skills
panel-damage-modifiers = Damage Modifiers
saving-throw = Saving Throw
panel-combat = Combat
panel-spellcasting = Spellcasting
panel-equipment = Equipment
panel-features = Features
panel-personality = Personality
panel-proficiencies = Proficiencies & Languages
panel-notes = Notes
btn-add-note = + Add Note
diff-notes-summary = "{ $text }" · { $level } · { $date } · total: { $count }

## Combat panel
armor-class = Armor Class
recalculate = Recalculate
rebuild = Rebuild
rebuild-confirm = The character will be rebuilt from scratch. HP, used slots, and hit dice are preserved.
toast-rebuild-done = Character rebuilt
toast-rebuild-skipped = { $count ->
    [one] Rebuilt with { $count } unknown feature preserved as-is. Open the build tab to review.
   *[other] Rebuilt with { $count } unknown features preserved as-is. Open the build tab to review.
}
toast-rebuild-removed = { $count ->
    [one] Rebuilt; { $count } obsolete feature removed (no current rules definition).
   *[other] Rebuilt; { $count } obsolete features removed (no current rules definition).
}
toast-rebuild-skipped-and-removed = Rebuilt; { $skipped ->
    [one] { $skipped } unknown feature preserved
   *[other] { $skipped } unknown features preserved
}, { $removed ->
    [one] { $removed } obsolete removed
   *[other] { $removed } obsolete removed
}. Open the build tab to review.
toast-rebuild-failed-class = Couldn’t rebuild: class definition “{ $name }” is missing. Re-pick the class in the character header.
toast-rebuild-failed-species = Couldn’t rebuild: species definition “{ $name }” is missing. Re-pick the species in the character header.
toast-rebuild-failed-background = Couldn’t rebuild: background definition “{ $name }” is missing. Re-pick the background in the character header.
toast-rebuild-failed-multiclass = Couldn’t rebuild: multiclass prerequisites for “{ $class }” aren’t met. Raise the required ability via Generation or an ASI feature in the Build tab.
toast-rebuild-action-open-build = Build
initiative = Initiative
speed = Speed
attack-count = Attacks
inspiration = Inspiration
proficiency-bonus = Proficiency Bonus
level = Level
class-level = Class Level
hit-dice = Hit Dice
hit-dice-max = Hit Dice (max)
hit-dice-used = Hit Dice (used)
hit-dice-sides = Hit Die size
caster-level = Caster Level
caster-ability = Casting Ability
caster-coef = Caster Type
caster-coef-full = Full
caster-coef-half = Half
caster-coef-third = Third
spell-slot = Slot
spell-slot-used = Slot Used
spell-slot-pool = Slot Pool
spell-cantrips = Cantrips
spell-known = Known Spells
spell-ready = Prepared Spells
hp = Health Points
current-hp = Current HP
hp-max = HP Max
temp-hp = Temp HP
successes = Successes
failures = Failures
short-rest = Short Rest
long-rest = Long Rest
drop-concentration = Drop Concentration
reset-stats = Reset

## Spellcasting panel
casting-ability = Ability
spell-save-dc = Save DC
spell-attack = Attack
spell-slots = Spell Slots
spells = Spells
spellbook = Spellbook
prepared-spells = Prepared Spells
spell-name = Spell name
free-uses = Free Uses

## Equipment panel
weapons = Weapons
name = Name
atk-magic = Magic
weapon-ability = Ability
attack = Attack
damage = Damage
heal = Heal
btn-add-weapon = + Add Weapon
btn-add-effect = Add Effect
armor = Armor
base-ac = Base AC
ac-formula = AC formula
btn-add-armor = + Add Armor
armor-type-light = Light
armor-type-medium = Medium
armor-type-heavy = Heavy
armor-type-shield = Shield
armor-type-natural = Natural
weapon-category-simple = Simple
weapon-category-martial = Martial
items = Items
item-name = Item name
qty = Qty
description = Description
btn-add-item = + Add Item
currency = Currency
spend = Spend
cast = Cast
gain = Gain
add-item = Add item

## Features / Personality / Proficiencies
feature-name = Feature name
btn-add-feature = Add Feature
source-class = Class
source-subclass = Subclass
source-species = Species
source-background = Background
source-user = Manual
history = History
personality-traits = Personality Traits
ideals = Ideals
bonds = Bonds
flaws = Flaws
proficiencies = Proficiencies
languages = Languages
language = Language
btn-add-language = + Add Language
tools = Tools
tool = Tool
btn-add-tool = + Add Tool
tool-count = Tool slots
used = Used
total = Total
max = Max
cost = Cost
btn-add-spell = + Add Spell
choose-option = Choose option
search = Search…
browse-options = Browse options
btn-add-option = + Add Option

## Abilities
ability-strength = Strength
ability-dexterity = Dexterity
ability-constitution = Constitution
ability-intelligence = Intelligence
ability-wisdom = Wisdom
ability-charisma = Charisma
ability-str = STR
ability-dex = DEX
ability-con = CON
ability-int = INT
ability-wis = WIS
ability-cha = CHA

## Skills
skill-acrobatics = Acrobatics
skill-animal-handling = Animal Handling
skill-arcana = Arcana
skill-athletics = Athletics
skill-deception = Deception
skill-history = History
skill-insight = Insight
skill-intimidation = Intimidation
skill-investigation = Investigation
skill-medicine = Medicine
skill-nature = Nature
skill-perception = Perception
skill-performance = Performance
skill-persuasion = Persuasion
skill-religion = Religion
skill-sleight-of-hand = Sleight of Hand
skill-stealth = Stealth
skill-survival = Survival

## Alignments
alignment-lawful-good = Lawful Good
alignment-neutral-good = Neutral Good
alignment-chaotic-good = Chaotic Good
alignment-lawful-neutral = Lawful Neutral
alignment-true-neutral = True Neutral
alignment-chaotic-neutral = Chaotic Neutral
alignment-lawful-evil = Lawful Evil
alignment-neutral-evil = Neutral Evil
alignment-chaotic-evil = Chaotic Evil

## Proficiencies
prof-light-armor = Light Armor
prof-medium-armor = Medium Armor
prof-heavy-armor = Heavy Armor
prof-shields = Shields
prof-simple-weapons = Simple Weapons
prof-martial-weapons = Martial Weapons

## Damage types
damage-acid = Acid
damage-bludgeoning = Bludgeoning
damage-cold = Cold
damage-fire = Fire
damage-force = Force
damage-lightning = Lightning
damage-necrotic = Necrotic
damage-piercing = Piercing
damage-poison = Poison
damage-psychic = Psychic
damage-radiant = Radiant
damage-slashing = Slashing
damage-thunder = Thunder

## Confirmation dialogs
confirm-reset = Reset character to blank?
remove-class = Remove class
confirm-remove-class = Remove this class and all its levels? You will need to rebuild the character afterwards.

## Session page
slot-level = Lv { $level }
session-actions = Spells and Weapons
session-stats = Main Stats
session-backpack = Backpack
session-resources = Resources
view-session = Session
view-editor = Editor
view-story = Story
tab-stats = Stats
tab-build = Build
tab-magic = Magic
tab-inventory = Inventory
tab-backstory = Backstory
story-new = New Story
story-prompt-placeholder = Describe what happened between sessions...
story-generate = Generate
story-stop = Stop
story-no-api-key = Configure your API key to generate stories.
story-settings = AI Settings
story-api-key = API Key
story-get-key = (get one)
story-model = Chat model
ai-settings-image-model = Image model
ai-settings-fetch-failed = Failed to load models
ai-settings-provider-hosted = Use hosted AI (Google sign-in required)
ai-settings-google-required = Sign in with Google to use hosted AI, or uncheck to paste your own key.
story-save = Save
story-delete = Delete
story-copy = Copy
story-error = Generation error
story-select = Select a story or create a new one
story-retry = Retry
ai-generate-title = AI Character Generator
ai-generate-description = Describe your character
ai-generate-placeholder = A brooding half-elf ranger who grew up in the wilderness...
ai-generate-button = AI Generate
ai-generate-no-key = Set your API key in the settings (gear icon below) to use AI generation.
ai-generate-phase-identity = Choosing identity...
ai-generate-phase-choices = Filling choices...
ai-generate-phase-retry = Fixing choices...
ai-generate-error = Generation error
level-up = Level Up
level-up-choose-class = Choose class to level up
session-cantrips = Cantrips
session-no-weapons = No weapons
session-no-items = No items
session-ability-mods = Ability Modifiers
session-saving-throws = Saving Throws
session-skills = Skills
session-tools = Tools
session-languages = Comprehensible Languages
session-damage-modifiers = Resistances
damage-vulnerability = Vulnerable
damage-resistance = Resistant
damage-immunity = Immune
damage-reduction = DR
action-type-action = Action
action-type-bonus-action = Bonus Action
action-type-reaction = Reaction
session-effects = Active Effects
effect-add = Add Effect
effect-remove = Remove Effect
effect-name = Effect name
effect-expr = Expression (optional)
effect-dice = Dice
effect-reroll = Reroll dice
roll-all-dice = Roll all dice
dice-rolls-title = Dice Rolls
btn-confirm = Confirm
apply-effect = Apply Effect

## Reference pages
ref-reference = Reference
ref-classes = Classes
ref-species = Species
ref-backgrounds = Backgrounds
ref-level = Level
ref-features = Features
ref-hit-die = Hit Die
ref-cantrips = Cantrips
ref-spells-known = Spells Known
ref-spells-ready = Spells Ready
ref-subclasses = Subclasses
ref-progression = Progression
ref-select-class = Select a class to view details
ref-select-species = Select a species to view details
ref-select-background = Select a background to view details
ref-search-feature = Search features...
feat-cat-class = Class Features
feat-cat-origin = Origin Feats
feat-cat-general = General Feats
feat-cat-fighting-style = Fighting Styles
feat-cat-epic-boon = Epic Boons
feat-cat-generation = Generation
feat-cat-faction = Faction
feat-cat-dragonmark = Dragonmark
feat-cat-system-species = Species (System)
feat-cat-system-background = Background (System)
feat-cat-system-subclass = Subclass (System)
feat-cat-system-class = Class (System)
feat-cat-all = All Categories
ref-spells = Spells
ref-select-spell-list = Select a spell list to view spells
ref-cantrips-level = cantrips
ref-spell-level = {$level ->
    [1] 1st level
    [2] 2nd level
    [3] 3rd level
    *[other] {$level}th level
    }
ref-spell-min-level = from level {$level}
ref-spell-always-ready = always ready
ref-spell-cast-time = Casting Time
ref-spell-range = Range
ref-spell-duration = Duration
ref-spell-concentration = Concentration
ref-spell-ritual = Ritual
ref-spell-range-self = Self
ref-spell-range-touch = Touch
ref-spell-range-feet = {$feet} ft.
ref-spell-duration-instant = Instantaneous
ref-spell-duration-rounds = {$rounds} {$rounds ->
    [one] round
   *[other] rounds
}
ref-spell-duration-minutes = {$minutes} min.
ref-spell-duration-hours = {$hours} hr.
ref-spell-duration-forever = Until dispelled
ref-spell-cast-rounds = {$rounds} rounds
ref-spell-cast-minutes = {$minutes} min.
ref-spell-cast-hours = {$hours} hr.
ref-spell-category = Category
spell-cat-damage = Damage
spell-cat-healing = Healing
spell-cat-buff = Buff
spell-cat-debuff = Debuff
spell-cat-control = Control
spell-cat-defense = Defense
spell-cat-utility = Utility
spell-cat-summon = Summon
spell-cat-social = Social
ref-prerequisites = Prerequisites
ref-spell-list-link = Spell List
expr-and = and
expr-or = or
expr-not = not

## Spell slot pools
pool-arcane = Arcane Slots
pool-pact = Pact Slots

## Quick Start
quick-start-title = Quick Start
quick-start-generation = Ability Scores
quick-start-create = Create Character
quick-start-skip = Skip


## Cloud sync
sync-disabled = Offline
sync-connecting = Connecting...
sync-synced = Synced
sync-syncing = Syncing...
sync-error = Sync error
sync-sign-in-google = Sign in with Google
show-expression = Show expression
points = Points
points-max = Max points
die-sides = Die sides
die-count = Die count
die-used = Used dice
bonus = Bonus
choice-count = Choice count
sticky = Always prepared
free-uses-used = Used free uses
reset = Reset
spell-level-badge = { $count ->
    [0] Cantrip
    [1] 1st level
    [2] 2nd level
    [3] 3rd level
   *[other] { $count }th level
}

## Avatar
avatar-change = Change portrait
avatar-remove = Remove portrait
avatar-generate = Generate with AI
avatar-load-failed = Failed to load image
avatar-generate-title = Generate avatar
avatar-generate-description = Extra details (optional)
avatar-generate-placeholder = pose, outfit, mood…
avatar-generate-button = Generate
avatar-generate-phase-rendering = Rendering image…
avatar-generate-failed = Failed to generate avatar
avatar-close = Close

# Enchantment modal
enchantment-edit = Edit enchantment
enchantment-charges = Charges
enchantment-charges-used = Used
enchantment-charges-max = Max
enchantment-passives = Passives
enchantment-actions = Actions
enchantment-no-passives = No passives
enchantment-no-actions = No actions
enchantment-action-name = Action name
enchantment-option-name = Mode name
assign-when = When
action-type = Type
effects = Effects
effect-range = Range
effect-duration = Duration
effect-scope = Scope
effect-stackable = Stackable
range-caster = Self
range-touch = Touch
range-feet = Feet
duration-instant = Instant
duration-rounds = Rounds
duration-forever = Permanent
charges = Charges
charges-max = Charges (max)
charges-used = Charges (used)
quantity = Quantity
equipped = Equipped
session-gear-actions = Gear actions
choice-cost = Cost
choice-consumes = Consumes
btn-add-passive = + Add passive
btn-add-action = + Add action
btn-save = Save
when-on-feature-add = On feature add
when-on-compute = On compute
when-on-gear-active = While active
when-on-effect = On effect
when-on-long-rest = On long rest
when-on-short-rest = On short rest
