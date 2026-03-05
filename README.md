# D&D 5e Character Sheet

A web-based D&D 5th Edition character sheet manager built with Rust and WebAssembly.

**[Live Demo](https://kstep.github.io/dnd-pc/)**

## Features

- Create, edit, and delete multiple characters
- Auto-save to browser localStorage with optional Firebase cloud sync
- Ability scores, modifiers, and saving throws
- Skills with proficiency tracking
- Combat stats (AC, HP, initiative, speed, hit dice)
- Spellcasting with multiple spell slot pools (Arcane, Pact) and spell lists
- Equipment and inventory management
- Multiclassing support with automatic class feature application
- Character summary view (read-only overview)
- Reference pages for classes, races, backgrounds, and spells
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

- Rust stable toolchain (nightly only needed for `cargo +nightly fmt`)
- `wasm32-unknown-unknown` target
- Trunk

```sh
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

## Cloud Sync (Firebase)

Cloud sync is optional. Without Firebase configuration the app works fully offline using localStorage.

To enable cross-device sync:

1. Create a [Firebase project](https://console.firebase.google.com/)

2. **Register a web app:**
   - In project settings (gear icon > **General**), scroll to **Your apps**
   - Click the web icon (`</>`) to add a web app
   - Copy the generated `firebaseConfig` object

3. **Enable Authentication:**
   - Go to **Authentication** > **Sign-in method**
   - Enable **Anonymous** provider
   - Enable **Google** provider

4. **Add authorized domains:**
   - Go to **Authentication** > **Settings** > **Authorized domains**
   - Ensure `localhost` is listed (for local development)
   - Add your deployment domain (e.g. `kstep.github.io`)

5. **Create Firestore database:**
   - Go to **Firestore Database** > **Create database**
   - Set security rules:

```
rules_version = '2';
service cloud.firestore {
  match /databases/{database}/documents {
    match /users/{userId}/characters/{charId} {
      allow read, write: if request.auth != null && request.auth.uid == userId;
    }
  }
}
```

6. **Add config to the app:**
   - Paste your Firebase config into `index.html`, replacing the placeholder values:

```js
const firebaseConfig = {
  apiKey: "YOUR_API_KEY",
  authDomain: "YOUR_PROJECT.firebaseapp.com",
  projectId: "YOUR_PROJECT_ID",
  storageBucket: "YOUR_PROJECT.firebasestorage.app",
  messagingSenderId: "YOUR_SENDER_ID",
  appId: "YOUR_APP_ID",
};
```

On startup the app signs in anonymously and pulls characters from Firestore. Edits are pushed automatically with a 2-second debounce. Clicking "Sign in with Google" links the anonymous account for cross-device access.

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
