#!/usr/bin/env python3
"""
Migrate expr formulas from legacy `@G._.SUFFIX` / `with(...)` syntax
to the new bare-group + `@.SUFFIX` model.

Transformations (applied in order):

1. `with(SCOPE, each(@, BODY))` → `each(SCOPE, BODY)`
2. `with(SCOPE, guard(COND, each(@, BODY)))`
   → `guard(COND', each(SCOPE, BODY'))` — guard kept outside iter;
   `COND` is rewritten so any bare `@` becomes the SCOPE group ref.
3. `with(SCOPE, BODY)` (no inner each) → `each(SCOPE, BODY')` — most
   `with` bodies are statement chains involving the bound group.
4. Group name rewrites: `@RESIST._`, `@VULN._`, `@IMMUNE._`,
   `@SKILL._.PROF`, `@SKILL._.ADV`, `@ABILITY.{MOD,SAVE,SAVE.PROF,
   SAVE.ADV,ADV}` rewritten according to context.

Run from the repo root.  Pass `--dry-run` to preview the diff.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path


# Group name → (new bare group, suffix-when-used-as-projection)
SUFFIX_GROUPS = {
    "@SKILL._.PROF": ("@SKILL", "PROF"),
    "@SKILL._.ADV": ("@SKILL", "ADV"),
    "@SKILL._": ("@SKILL", ""),
    "@RESIST._": ("@DMG", ""),
    "@VULN._": ("@DMG", "VULN"),
    "@IMMUNE._": ("@DMG", "IMMUNE"),
    "@ABILITY.SAVE.PROF": ("@ABILITY", "SAVE.PROF"),
    "@ABILITY.SAVE.ADV": ("@ABILITY", "SAVE.ADV"),
    "@ABILITY.SAVE": ("@ABILITY", "SAVE"),
    "@ABILITY.MOD": ("@ABILITY", "MOD"),
    "@ABILITY.ADV": ("@ABILITY", "ADV"),
}


def find_matching(text: str, start: int) -> int:
    """Given start of `(`, find matching `)`. Returns index of `)`."""
    depth = 0
    i = start
    while i < len(text):
        c = text[i]
        if c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    raise ValueError(f"unbalanced parens at offset {start}: {text!r}")


def split_top_comma(text: str) -> list[str]:
    """Split on top-level commas, respecting paren nesting."""
    parts = []
    depth = 0
    start = 0
    for i, c in enumerate(text):
        if c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
        elif c == "," and depth == 0:
            parts.append(text[start:i].strip())
            start = i + 1
    parts.append(text[start:].strip())
    return parts


def parse_scope(text: str, start: int) -> tuple[str, str | None, int]:
    """Parse `@G` or `@G(mask)` starting at index `start`.
    Returns (group_name, mask_or_None, end_offset).
    `group_name` includes leading `@`."""
    assert text[start] == "@"
    i = start + 1
    while i < len(text) and (text[i].isalnum() or text[i] in "._"):
        i += 1
    grp = text[start:i]
    mask = None
    if i < len(text) and text[i] == "(":
        end = find_matching(text, i)
        mask = text[i + 1 : end]
        i = end + 1
    return grp, mask, i


def rewrite_scope_group(grp: str) -> str:
    """Rewrite group ref appearing in scope position (each/fold arg)."""
    if grp in SUFFIX_GROUPS:
        return SUFFIX_GROUPS[grp][0]
    # bare @G unchanged
    return grp


def rewrite_body_token(grp: str, scope_suffix: str) -> str:
    """Rewrite a group-ref occurring in body context.
    `scope_suffix`: the suffix tied to the enclosing scope (e.g. "PROF"
    when scope is @SKILL._.PROF). Bare `@G_full` matches scope → `@`
    or `@.SUFFIX`; other suffixed forms emit `@.SUFFIX`."""
    if grp in SUFFIX_GROUPS:
        new_grp, suffix = SUFFIX_GROUPS[grp]
        if suffix == scope_suffix:
            return "@" if not suffix else f"@.{suffix}"
        return f"@.{suffix}" if suffix else "@"
    return grp


def rewrite_inline_refs(body: str, scope_grp_full: str, scope_grp_bare: str, scope_suffix: str) -> str:
    """Walk `body`, replacing legacy group references in body context.

    `scope_grp_full`: the original groupref like `@SKILL._.PROF`.
    `scope_grp_bare`: the new bare groupref like `@SKILL`.
    `scope_suffix`: the suffix associated with the scope's primary column
    (e.g. `PROF` for `@SKILL._.PROF`)."""
    # Sort longest-first to avoid partial matches.
    keys = sorted(SUFFIX_GROUPS.keys(), key=len, reverse=True)

    out = []
    i = 0
    while i < len(body):
        c = body[i]
        if c == "@":
            # Find longest matching suffix-group at this position.
            matched = None
            for k in keys:
                if body[i : i + len(k)] == k:
                    # Ensure next char isn't an alpha char (avoid partial @ABILITY.MOD on @ABILITY.MODX).
                    j = i + len(k)
                    next_c = body[j] if j < len(body) else ""
                    if next_c.isalnum() or next_c in "._":
                        continue
                    matched = k
                    break
            if matched is not None:
                new_grp, suffix = SUFFIX_GROUPS[matched]
                # In body context, suffixed group → projection.
                # If matches scope's primary, just `@` (bare).
                if matched == scope_grp_full or (
                    new_grp == scope_grp_bare and suffix == scope_suffix
                ):
                    out.append("@" if not suffix else f"@.{suffix}")
                else:
                    out.append("@" if not suffix else f"@.{suffix}")
                i += len(matched)
                continue
            # Walk to capture potential group name.
            j = i + 1
            while j < len(body) and (body[j].isalnum() or body[j] == "_"):
                j += 1
            grp_text = body[i:j]
            # Bare `@` (no following ident): under old code it meant
            # the with-binding's current iter member. Under new model
            # body uses `@` for primary column. If scope had a suffix
            # (e.g. @SKILL._.PROF), bare `@` projects onto that suffix.
            if grp_text == "@":
                out.append(f"@.{scope_suffix}" if scope_suffix else "@")
            elif grp_text == scope_grp_full:
                # Body had the full legacy ref (e.g. `@SKILL._.PROF`):
                # convert to projection.
                out.append(f"@.{scope_suffix}" if scope_suffix else "@")
            else:
                # `@SKILL`, `@ARG`, other refs — leave as-is. They
                # might be subgroup-position refs we already canonicalised.
                out.append(grp_text)
            i = j
        else:
            out.append(c)
            i += 1
    return "".join(out)


def collapse_with(expr: str) -> str:
    """Find `with(SCOPE, BODY)` and rewrite. Recursive (handles nested)."""
    while True:
        idx = expr.find("with(")
        if idx < 0:
            return expr
        # Parse SCOPE
        scope_start = idx + len("with(")
        scope_grp_full, scope_mask, after_scope = parse_scope(expr, scope_start)
        # Expect a comma
        if not (after_scope < len(expr) and expr[after_scope] == ","):
            raise ValueError(f"expected ',' after with-scope at {after_scope}: {expr!r}")
        body_start = after_scope + 1
        # Find matching `)` for `with(`
        with_close = find_matching(expr, idx + len("with") )
        body = expr[body_start:with_close].strip()

        scope_bare = rewrite_scope_group(scope_grp_full)
        scope_suffix = ""
        if scope_grp_full in SUFFIX_GROUPS:
            scope_suffix = SUFFIX_GROUPS[scope_grp_full][1]

        # Build new scope str (for use in subgroup positions).
        scope_str = scope_bare
        if scope_mask is not None:
            scope_str = f"{scope_bare}({scope_mask})"

        # FIRST: in subgroup-arg positions (each(@, ...), fold(op, @, ...)),
        # bare `@` referenced the with-binding scope. Substitute SCOPE
        # before rewriting body-context refs (otherwise the body-context
        # pass would rewrite `@` here to `@.SUFFIX`).
        body = re.sub(r"each\(@,\s*", f"each({scope_str}, ", body)
        body = re.sub(
            r"(fold\([^,]+,\s*)@(\s*,)",
            rf"\1{scope_str}\2",
            body,
        )

        # THEN: rewrite remaining body-context legacy references.
        body_rewritten = rewrite_inline_refs(
            body, scope_grp_full, scope_bare, scope_suffix
        )

        # If body doesn't contain `each(` at top level yet, wrap in each.
        # Heuristic: if top-level pattern is `each(SCOPE, ...)` already, leave.
        # If body starts with `guard(...)` and inside has `each(SCOPE, ...)`, leave too.
        # Otherwise wrap.
        if not body_rewritten.startswith("each(") and not body_rewritten.startswith("guard("):
            body_rewritten = f"each({scope_str}, {body_rewritten})"

        # Substitute back
        expr = expr[:idx] + body_rewritten + expr[with_close + 1 :]


def rewrite_standalone(expr: str) -> str:
    """Rewrite standalone `each(@G.SUFFIX, BODY)` (no with) — body's
    `@G.SUFFIX` becomes `@.SUFFIX`."""
    pattern = re.compile(r"\b(each|fold)\(")
    # Walk each `each(`/`fold(` start, find scope, rewrite scope + body.
    i = 0
    out = []
    while i < len(expr):
        m = pattern.search(expr, i)
        if not m:
            out.append(expr[i:])
            break
        # Append everything up to the match
        out.append(expr[i : m.start()])
        kw_start = m.start()
        kw_end = m.end()
        # For fold, skip the op arg (e.g. `fold(+, ...)`)
        out.append(expr[kw_start:kw_end])
        i = kw_end
        if m.group(1) == "fold":
            # Skip `op, ` part. Find first comma.
            depth = 0
            j = i
            while j < len(expr) and not (expr[j] == "," and depth == 0):
                if expr[j] == "(":
                    depth += 1
                elif expr[j] == ")":
                    depth -= 1
                j += 1
            out.append(expr[i : j + 1])
            i = j + 1
            # Skip whitespace
            while i < len(expr) and expr[i].isspace():
                out.append(expr[i])
                i += 1
        # Now parse the scope at position i (if it starts with @)
        if i < len(expr) and expr[i] == "@":
            scope_grp_full, scope_mask, after_scope = parse_scope(expr, i)
            scope_bare = rewrite_scope_group(scope_grp_full)
            scope_suffix = SUFFIX_GROUPS.get(scope_grp_full, ("", ""))[1]
            scope_str = scope_bare
            if scope_mask is not None:
                scope_str = f"{scope_bare}({scope_mask})"
            out.append(scope_str)
            i = after_scope
            # Expect `, BODY)`
            if i < len(expr) and expr[i] == ",":
                # Find matching close paren for the keyword's open paren
                # Walk back to find which `(` was opened for this each/fold.
                # We're at depth 1 inside that paren. Walk forward to depth 0.
                close = find_matching(expr, kw_end - 1)
                body = expr[i + 1 : close]
                body_rewritten = rewrite_inline_refs(
                    body, scope_grp_full, scope_bare, scope_suffix
                )
                out.append(", ")
                out.append(body_rewritten)
                out.append(")")
                i = close + 1
            else:
                # Just continue — shouldn't normally happen.
                pass
        # Continue search from i.
    return "".join(out)


def migrate_expr(expr: str) -> str:
    """Apply full migration pipeline to a single expression."""
    if "with(" in expr:
        expr = collapse_with(expr)
    # Continue with any remaining patterns
    # (each(@G.SUFFIX, ...) and standalone forms)
    expr = rewrite_standalone(expr)
    return expr


# Walk JSON files in catalog dirs and migrate every "expr" string.
def migrate_file(path: Path, dry_run: bool = False) -> int:
    """Returns count of changed expressions."""
    text = path.read_text(encoding="utf-8")
    changed = 0

    def repl(match):
        nonlocal changed
        prefix = match.group(1)
        body = match.group(2)
        suffix = match.group(3)
        # Unescape JSON string: simple replace of \" with " for processing.
        unescaped = body.replace('\\"', '"')
        migrated = migrate_expr(unescaped)
        if migrated != unescaped:
            changed += 1
        # Re-escape
        escaped = migrated.replace('"', '\\"')
        return f"{prefix}{escaped}{suffix}"

    pattern = re.compile(r'("expr"\s*:\s*")((?:[^"\\]|\\.)*)(")')
    new_text = pattern.sub(repl, text)
    if changed > 0 and not dry_run:
        path.write_text(new_text, encoding="utf-8")
    return changed


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("paths", nargs="*", default=["public/data"])
    args = parser.parse_args()

    total = 0
    files_changed = 0
    for root in args.paths:
        for path in Path(root).rglob("*.json"):
            n = migrate_file(path, args.dry_run)
            if n > 0:
                files_changed += 1
                total += n
                print(f"{path}: {n} expressions changed")
    print(f"\nTotal: {total} expressions changed across {files_changed} files")


if __name__ == "__main__":
    main()
