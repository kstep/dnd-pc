#!/usr/bin/env bash
# One-shot GCP setup for the dnd-pc proxy. Idempotent — re-running is safe;
# existing resources log a warning and are skipped.
#
# Reads backend/.env if present — pulls OPENAI_API_KEY and
# FIREBASE_PROJECT_ID from there. Override anything via env vars.
#
# Usage:
#   # Minimum — assumes backend/.env has OPENAI_API_KEY + FIREBASE_PROJECT_ID:
#   REGION=europe-west3 GITHUB_REPO=kstep/dnd-pc ./backend/setup-gcp.sh
#
# At the end it prints the values you need to paste into GitHub Secrets.

set -euo pipefail

ENV_FILE="$(dirname "$0")/.env"
if [[ -f "$ENV_FILE" ]]; then
  echo "Loading $ENV_FILE"
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
fi

PROJECT_ID="${PROJECT_ID:-${FIREBASE_PROJECT_ID:-}}"

: "${PROJECT_ID:?Set PROJECT_ID or FIREBASE_PROJECT_ID (in backend/.env)}"
: "${REGION:?Set REGION (e.g. europe-west3)}"
: "${GITHUB_REPO:?Set GITHUB_REPO (owner/repo, e.g. kstep/dnd-pc)}"
: "${OPENAI_API_KEY:?Set OPENAI_API_KEY (put it in backend/.env)}"

SA_NAME=dnd-pc-proxy-deploy
SA_EMAIL=$SA_NAME@$PROJECT_ID.iam.gserviceaccount.com
PROJECT_NUMBER=$(gcloud projects describe "$PROJECT_ID" --format='value(projectNumber)')

echo "== Enabling APIs =="
gcloud services enable \
  run.googleapis.com \
  cloudbuild.googleapis.com \
  artifactregistry.googleapis.com \
  secretmanager.googleapis.com \
  iamcredentials.googleapis.com \
  --project="$PROJECT_ID"

echo "== Storing OpenAI key in Secret Manager =="
if gcloud secrets describe openai-api-key --project="$PROJECT_ID" >/dev/null 2>&1; then
  echo "  secret 'openai-api-key' exists — adding a new version"
  printf %s "$OPENAI_API_KEY" | gcloud secrets versions add openai-api-key \
    --data-file=- --project="$PROJECT_ID"
else
  printf %s "$OPENAI_API_KEY" | gcloud secrets create openai-api-key \
    --data-file=- --project="$PROJECT_ID"
fi

echo "== Creating deploy SA =="
if ! gcloud iam service-accounts describe "$SA_EMAIL" --project="$PROJECT_ID" >/dev/null 2>&1; then
  gcloud iam service-accounts create "$SA_NAME" --project="$PROJECT_ID"
else
  echo "  SA $SA_EMAIL exists — skipping"
fi

echo "== Granting roles to deploy SA =="
for role in \
  roles/run.admin \
  roles/cloudbuild.builds.editor \
  roles/artifactregistry.writer \
  roles/storage.admin \
  roles/iam.serviceAccountUser; do
  gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:$SA_EMAIL" --role="$role" --condition=None >/dev/null
  echo "  + $role"
done

echo "== Granting secret-accessor to runtime SA =="
gcloud secrets add-iam-policy-binding openai-api-key \
  --member="serviceAccount:${PROJECT_NUMBER}-compute@developer.gserviceaccount.com" \
  --role=roles/secretmanager.secretAccessor --project="$PROJECT_ID" --condition=None >/dev/null

echo "== Workload Identity Federation =="
if ! gcloud iam workload-identity-pools describe github \
  --location=global --project="$PROJECT_ID" >/dev/null 2>&1; then
  gcloud iam workload-identity-pools create github \
    --location=global --project="$PROJECT_ID"
else
  echo "  pool 'github' exists — skipping"
fi

if ! gcloud iam workload-identity-pools providers describe github-provider \
  --location=global --workload-identity-pool=github \
  --project="$PROJECT_ID" >/dev/null 2>&1; then
  gcloud iam workload-identity-pools providers create-oidc github-provider \
    --location=global --workload-identity-pool=github \
    --issuer-uri=https://token.actions.githubusercontent.com \
    --attribute-mapping='google.subject=assertion.sub,attribute.repository=assertion.repository,attribute.repository_owner=assertion.repository_owner' \
    --attribute-condition="attribute.repository_owner == '${GITHUB_REPO%%/*}'" \
    --project="$PROJECT_ID"
else
  echo "  provider 'github-provider' exists — skipping"
fi

POOL=projects/$PROJECT_NUMBER/locations/global/workloadIdentityPools/github

gcloud iam service-accounts add-iam-policy-binding "$SA_EMAIL" \
  --role=roles/iam.workloadIdentityUser \
  --member="principalSet://iam.googleapis.com/$POOL/attribute.repository/$GITHUB_REPO" \
  --project="$PROJECT_ID" --condition=None >/dev/null

cat <<EOF

== Done. Put these into GitHub → Settings → Secrets and variables → Actions → Variables tab ==
(Not Secrets — none of these are sensitive. Using Variables keeps them
 readable in CI logs, which helps debugging.)

  GCP_PROJECT_ID       = $PROJECT_ID
  GCP_WIF_PROVIDER     = $POOL/providers/github-provider
  GCP_SA_EMAIL         = $SA_EMAIL
  CLOUD_RUN_REGION     = $REGION
  FIREBASE_API_KEY     = AIzaSyAZmoMWsdVZKzJL5Xl_am_j7RmFAD4ehL8
  ALLOWED_ORIGINS      = https://kstep.me

OPENAI_API_KEY stays in GCP Secret Manager (already seeded above), not
in GitHub — the Cloud Run service picks it up via --set-secrets.

Then merge the branch — .github/workflows/deploy-backend.yml will deploy.
EOF
