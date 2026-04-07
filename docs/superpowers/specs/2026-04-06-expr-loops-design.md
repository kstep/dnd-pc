# Expr Loops: Each/Next/Fold Design

## Problem

features.json contains highly repetitive expression patterns:
- 6-ability ASI templates (37+ features)
- 18-skill selection templates (5+ features)
- Mass resistance assignments
- Combined ability + save proficiency patterns

## Solution

Add loop constructs to Expr: `each` for side-effect loops, `fold` for reduction. Both built on 4 new Op variants + existing EvalIf/Eval/BinOp.

## New Op Variants

```rust
Each(Grp)        // init: push 0 to iter_stack, push 1 if non-empty / 0 if empty
Next(Grp)        // advance: inc iter_stack top, push 1 if more / pop+push 0 if done
PushGroup(Grp)   // read: push group.member(iter_stack.last()) via ctx.resolve
AssignGroup(Grp) // write: pop value, assign to group.member(iter_stack.last())
```

Each/Next push boolean → reuse existing EvalIf for control flow.

## Opcode Patterns

### each
```
each($ABILITY, body):
  [Each(Ability), EvalIf(m, NOOP)]
  Block m: [...body..., Next(Ability), EvalIf(m, NOOP)]
```

### fold (no dedicated Op — recursive Eval + BinOp)
```
fold(+, $ABILITY, expr):
  [Each(Ability), EvalIf(m, NOOP)]
  Block m: [...expr..., Next(Ability), EvalIf(n, NOOP)]
  Block n: [Eval(m), BinOp(Add)]
```

fold works via recursion: each iteration pushes a value, recursive Eval(m) computes the rest, BinOp accumulates on unwind.

## Generic Trait

```rust
pub trait VarGroup<Var>: Copy + Eq + Debug + Serialize + DeserializeOwned + FromStr {
    fn member(self, index: usize) -> Option<Var>;
}
```

## Domain Implementation: AttributeGroup

```rust
enum AttributeGroup {
    Ability,         // ABILITY          → Ability(Str..Cha) [6]
    AbilityMod,      // ABILITY.MOD      → Modifier(Str..Cha) [6]
    AbilitySave,     // ABILITY.SAVE     → SavingThrow(Str..Cha) [6]
    AbilitySaveProf, // ABILITY.SAVE.PROF → SaveProficiency(Str..Cha) [6]
    AbilitySaveAdv,  // ABILITY.SAVE.ADV → SaveAdvantage(Str..Cha) [6]
    AbilityAdv,      // ABILITY.ADV      → AbilityAdvantage(Str..Cha) [6]
    Skill,           // SKILL._          → Skill(Acro..Surv) [18]
    SkillProf,       // SKILL._.PROF     → SkillProficiency(Acro..Surv) [18]
    SkillAdv,        // SKILL._.ADV      → SkillAdvantage(Acro..Surv) [18]
    Resist,          // RESIST._         → Resistance(Acid..Thunder) [13]
    Vuln,            // VULN._           → Vulnerability(Acid..Thunder) [13]
    Immune,          // IMMUNE._         → Immunity(Acid..Thunder) [13]
    Arg,             // ARG              → Arg(i), unbounded companion
}
```

## Syntax

`$` prefix distinguishes group references from variable references:

```
each($ABILITY, if($ABILITY < 20, $ABILITY += $ARG))
fold(and, $ABILITY, in($ARG, 0, 2) and $ABILITY + $ARG <= 20)
fold(+, $ABILITY, $ARG) == 2
each($RESIST._, $RESIST._ = 1)
```

## Expression Examples

### Ability Score Improvement
```
guard(
  fold(and, $ABILITY, in($ARG, 0, 2) and $ABILITY + $ARG <= 20)
  and fold(+, $ABILITY, $ARG) == 2,
  each($ABILITY, if($ABILITY < 20, $ABILITY += $ARG)))
```

### Skilled (18 skills)
```
guard(
  fold(and, $SKILL._.PROF, in($ARG, 0, 1)) and fold(+, $SKILL._.PROF, $ARG) == 3,
  each($SKILL._.PROF, if($SKILL._.PROF == 0, $SKILL._.PROF += $ARG)))
```

### Resilient (ability + save prof)
```
guard(
  fold(and, $ABILITY, in($ARG, 0, 1)) and fold(+, $ABILITY, $ARG) == 1,
  each($ABILITY,
    if($ABILITY < 20, $ABILITY += $ARG);
    $ABILITY.SAVE.PROF = max($ABILITY.SAVE.PROF, $ARG)))
```

### Shadowy Form (mass resist)
```
each($RESIST._, $RESIST._ = 1)
```

### Point Buy
```
guard(
  fold(and, $ABILITY, in($ARG, 0, 7))
  and fold(+, $ABILITY, $ARG + max(0, $ARG - 5)) == 27,
  each($ABILITY, $ABILITY += $ARG))
```
