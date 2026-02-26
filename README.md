# D&D 5e Character Sheet

A web-based D&D 5th Edition character sheet manager built with Rust and WebAssembly.

**[Live Demo](https://kstep.github.io/dnd-pc/)**

## Features

- Create, edit, and delete multiple characters
- Auto-save to browser localStorage
- Ability scores, modifiers, and saving throws
- Skills with proficiency tracking
- Combat stats (AC, HP, initiative, speed, hit dice)
- Spellcasting with spell slots and spell lists
- Equipment and inventory management
- Multiclassing support with automatic class feature application
- Share characters via compressed URL
- JSON import/export
- Internationalization (English and Russian)
- PWA with offline support

## Tech Stack

- [Leptos 0.8](https://github.com/leptos-rs/leptos) — reactive Rust web framework (CSR mode)
- [Trunk](https://trunkrs.dev/) — WASM bundler and dev server
- SCSS with [Open Props](https://open-props.style/) design tokens
- [leptos-fluent](https://github.com/mondeja/leptos-fluent) — i18n via Fluent
- `gloo-storage` — localStorage persistence
- `postcard` + `brotli` + `base64` — character sharing pipeline

## Getting Started

### Prerequisites

- Rust nightly toolchain
- `wasm32-unknown-unknown` target
- Trunk

```sh
rustup toolchain install nightly --allow-downgrade
rustup target add wasm32-unknown-unknown
cargo install trunk
```

### Development

```sh
trunk serve --port 3000 --open
```

Opens the app at `http://localhost:3000` with hot reload.

### Linting and Formatting

```sh
cargo clippy
cargo +nightly fmt
```

## Building & Deployment

### Release Build

```sh
trunk build --release
```

Outputs static files to the `dist/` directory.

### GitHub Pages

The project deploys automatically via GitHub Actions (`.github/workflows/deploy.yml`):

```sh
trunk build --release --public-url /dnd-pc/
```
