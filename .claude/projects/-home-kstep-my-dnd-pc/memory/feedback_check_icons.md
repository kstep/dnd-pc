---
name: Always check icons.svg
description: Before using any Lucide icon in code, verify it exists in public/icons.svg and add it if missing
type: feedback
---

Always check that icons exist in `public/icons.svg` before using them in code. If an icon is missing, download it from Lucide (`https://unpkg.com/lucide-static@latest/icons/{name}.svg`) and add it as a `<symbol>` to icons.svg.

**Why:** Icons are loaded from a local sprite sheet, not from CDN. Missing icons render as invisible/broken elements with no error. This has caused wasted debugging time.

**How to apply:** Before any PR/commit that introduces new `<Icon name="..." />` usage, grep `icons.svg` for `id="icon-{name}"`. If not found, fetch from Lucide and add.
