# tier() Function — Design Spec

## Problem

13 expressions in features.json use deeply nested `if(X >= N, val, if(X >= M, ...))` chains for level-based threshold lookups. Hard to read and maintain.

## Syntax

```
tier(var, threshold:value, threshold:value, ...)
```

- Thresholds in ascending order, colon-separated pairs
- Semantics: find the largest threshold <= var, return its value
- If var < first threshold → 0

### Examples

```
# Before:
if(CLASS_LEVEL >= 17, 4, if(CLASS_LEVEL >= 11, 3, if(CLASS_LEVEL >= 5, 2, 1)))d8

# After:
tier(CLASS_LEVEL, 1:1, 5:2, 11:3, 17:4)d8

# Other:
tier(CLASS_LEVEL, 1:2, 10:3, 14:4)d6
HP += tier(CLASS_LEVEL, 1:1, 3:2, 5:3, 10:4, 15:5)d6
TEMP_HP = max(TEMP_HP, tier(CLASS_LEVEL, 1:5, 10:10, 15:15))
```

## RPN Compilation

Pairs pushed as constants, variable last, then `Tier(count)`:

```
PushConst(1) PushConst(1)    // threshold:value pair 1
PushConst(5) PushConst(2)    // pair 2
PushConst(11) PushConst(3)   // pair 3
PushConst(17) PushConst(4)   // pair 4
PushVar(CLASS_LEVEL)          // variable to compare
Tier(4)                       // pop var, pop 4*2 values, push result
```

`Op::Tier(Val)` where Val = pair count. Op stays Copy.

## Evaluation

```
fn eval_tier(stack, count):
    var = stack.pop()
    result = 0
    for i in 0..count:
        value = stack.pop()
        threshold = stack.pop()
        if var >= threshold:
            result = value
    push(result)
```

Pop pairs in reverse (LIFO), so we iterate from highest threshold to lowest. First match where `var >= threshold` wins — but since we pop highest first, we need to track: keep updating result for every threshold that matches (since lower thresholds also match). Simpler: pop all pairs into temp array, scan ascending.

Or: pop in reverse, remember last match:
```
var = pop()
result = 0
for i in (0..count).rev():  // pairs are on stack highest-first
    value = pop()
    threshold = pop()
    if var >= threshold:
        result = value       // keep overwriting — last (highest matching) wins
push(result)
```

Wait — stack is LIFO so popping gives us pairs in reverse order (highest threshold first). We want the highest matching threshold. So:

```
var = pop()
result = 0
found = false
for _ in 0..count:
    value = pop()       // popped in reverse: highest pair first
    threshold = pop()
    if !found && var >= threshold:
        result = value
        found = true
push(result)
```

First match (highest threshold) is the answer. Must still pop remaining pairs to clean the stack.

## Formatter (Display)

`Tier(n)` → peek at preceding n*2 constants + 1 variable on the output stack. Render as `tier(var, t1:v1, t2:v2, ...)`.

## Files to Modify

- `src/expr/ops.rs` — add `Op::Tier(Val)`, stack_delta
- `src/expr/tokenizer.rs` — add `Token::Colon`
- `src/expr/parser.rs` — parse `tier(var, t:v, t:v, ...)`
- `src/expr/interpret/evaluator.rs` — eval logic
- `src/expr/interpret/formatter.rs` — Display round-trip
- `src/expr/interpret/analyze.rs` — pass-through (pop/push stack balance)
- `src/expr/interpret/mod.rs` — eval_op for Tier
- `src/expr/de.rs` — postcard deserialization (if Op is deserialized)
- `public/data/features.json` — convert 13 if-chains
- Custom interpreters: `ArgSummarizer` in `ai.rs`, `AssignmentSummarizer` in `pages/reference/mod.rs`, `FormBuilder` in `components/expr_args_input.rs`
