# dnd-pc proxy

Thin OpenAI proxy for the dnd-pc web app. Runs on Google Cloud Run.

Holds the OpenAI API key server-side (Secret Manager) and forwards
requests from authenticated Firebase Google users to OpenAI.

## Endpoints

- `ANY /v1/{*path}` → `https://api.openai.com/v1/{path}`. Streams the
  upstream response body verbatim (SSE works). Requires
  `Authorization: Bearer <firebase-id-token>` with `sign_in_provider =
  google.com` **and** an email listed in the whitelist (see below).
  Everything else gets 403 `{"error":"google_required"}` or
  `{"error":"email_not_allowed"}`.
- `GET /d/{uid}/{char_id}/character.json` → character JSON. Bearer token
  is **optional**: owner sees their own private chars, anyone sees chars
  with `shared == true`. Access control is delegated to `firestore.rules`.
- `GET /d/{uid}/{char_id}/avatar.webp` → decoded avatar image. Fetches
  the `/users/{uid}/avatars/{char_id}` doc, strips the `data:<mime>;base64,`
  prefix, returns the raw bytes with the original mime type. Avatars are
  public-read (`allow read: if true;` in `firestore.rules`).

## Whitelist

The proxy **denies everyone by default**. To grant access, add emails to
the Firestore doc `config/proxy`:

```json
{ "allowed_emails": ["milezv@gmail.com", "friend@gmail.com"] }
```

- Edit in Firebase Console: Firestore Data → `config/proxy` →
  `allowed_emails` array.
- Changes are picked up within **60 seconds** (cache TTL). No service
  restart needed.
- Empty array, missing doc, or typo → deny-all. Your own email stops
  working if you break the list, which is the fast feedback.
- If Firestore is unreachable *and* the proxy has no cached list, `/v1/*`
  returns `503 {"error":"config_unavailable"}`.

First-time setup: the `firestore.rules` entry for `config/{id}` lets the
owner uid write and anyone read. Replace `REPLACE_WITH_OWNER_UID` in
`firestore.rules` with your Firebase uid before `firebase deploy
--only firestore:rules`.

## Env vars

| Name                   | Required | Notes                                     |
| ---------------------- | -------- | ----------------------------------------- |
| `OPENAI_API_KEY`       | yes      | from Secret Manager in production         |
| `FIREBASE_PROJECT_ID`  | yes      | Firebase project id for JWT iss/aud check |
| `FIREBASE_API_KEY`     | yes      | public Firebase Web API key; needed for unauthenticated Firestore reads (shared chars) |
| `ALLOWED_ORIGINS`      | no       | comma-separated (default `http://localhost:3000`) |
| `PORT`                 | no       | provided by Cloud Run; default 8080       |
| `RUST_LOG`             | no       | default `info`                            |

## Local dev

Copy `backend/.env.example` to `backend/.env` and fill in the values —
`dotenvy` loads it automatically on startup.

```sh
cp backend/.env.example backend/.env
$EDITOR backend/.env

cargo run -p dnd-pc-proxy
```

Alternatively export the vars directly instead of using a `.env` file.

Test:

```sh
curl http://localhost:8080/v1/models  # -> 401 {"error":"unauthorized"}
```

Full end-to-end: run the frontend against this proxy. `trunk` doesn't
read `.env`, so export the vars before launching it:

```sh
set -a; source backend/.env; set +a
trunk serve --port 3000 --open
```

`PROXY_URL` is baked into the WASM bundle via `option_env!` at compile
time. Restart `trunk` after changing it. `FIREBASE_PROJECT_ID` on the
backend must match the one the frontend SDK initializes with, otherwise
JWT `iss`/`aud` won't match.

## Run tests

```sh
cargo test -p dnd-pc-proxy
```

## Deploy to Cloud Run

One-time setup:

```sh
# 1. Enable APIs
gcloud services enable run.googleapis.com \
    secretmanager.googleapis.com \
    artifactregistry.googleapis.com

# 2. Store the OpenAI key
printf %s "$OPENAI_API_KEY" | gcloud secrets create openai-api-key --data-file=-

# 3. Grant the Cloud Run runtime SA access
PROJECT_NUMBER=$(gcloud projects describe "$PROJECT_ID" --format='value(projectNumber)')
gcloud secrets add-iam-policy-binding openai-api-key \
    --member="serviceAccount:${PROJECT_NUMBER}-compute@developer.gserviceaccount.com" \
    --role=roles/secretmanager.secretAccessor
```

Deploy (manual):

```sh
gcloud run deploy dnd-pc-proxy \
    --source . \
    --region europe-west3 \
    --set-env-vars FIREBASE_PROJECT_ID=dnd-pc-d6388,ALLOWED_ORIGINS=https://kstep.github.io \
    --set-secrets OPENAI_API_KEY=openai-api-key:latest \
    --allow-unauthenticated \
    --max-instances 3 \
    --cpu 1 \
    --memory 256Mi
```

Note the Dockerfile expects the build context to be the **repo root** (so it
can see the workspace `Cargo.toml`). The CI workflow
`.github/workflows/deploy-backend.yml` runs `docker build -f
backend/Dockerfile .` from the repo root.

## Build Cargo / Docker

Local container build:

```sh
# From repo root
docker build -f backend/Dockerfile -t dnd-pc-proxy:dev .
docker run --rm -p 8080:8080 \
    -e OPENAI_API_KEY=sk-... \
    -e FIREBASE_PROJECT_ID=... \
    dnd-pc-proxy:dev
```
