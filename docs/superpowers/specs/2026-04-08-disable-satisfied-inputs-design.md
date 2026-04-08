# Disable Satisfied ARG Inputs — Design Spec

## Problem

When a guard expression is satisfied (e.g. user picked 3 of 18 skills), remaining unfilled inputs stay enabled. User can accidentally select more than needed.

## Solution

Pass `is_valid: Memo<bool>` into `FormCtx`. Each ARG input (checkbox/number) gets `prop:disabled = is_valid && value == 0`.

Reactive: checking a 4th checkbox when 3 required → guard becomes false → `is_valid` = false → all inputs re-enable → user can fix.

## Changes

`src/components/expr_args_input.rs`:

1. Add `is_valid: Memo<bool>` field to `FormCtx`
2. `push_arg_input` passes `ctx.is_valid` to `arg_checkbox`/`arg_number`
3. `arg_checkbox(signal, is_valid)` → `prop:disabled=move || is_valid.get() && signal.get() == 0`
4. `arg_number(signal, is_valid)` → same
5. Where `FormCtx` is constructed in `ExprArgsInput`, pass `is_valid`

## Files

- `src/components/expr_args_input.rs`
