#!/usr/bin/env bash
# Build the glogger.Personal NSIS installer locally.
#
# src-tauri/tauri.personal.conf.json sets bundle.createUpdaterArtifacts:true so
# CI publishes signed updater artifacts, and the Tauri CLI hard-fails without a
# signing key when that is on. A *local* installer never needs a signature (the
# updater only verifies the artifact it downloads), so unless a key is already
# in the environment we turn updater artifacts off for this build instead of
# requiring the release key on this machine.
#
# For a signed local build (matching CI):
#   TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/glogger-personal.key)" \
#   TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat ~/.tauri/glogger-personal.key.password)" \
#   npm run tauri:build:personal
# Key material lives outside the repo at ~/.tauri/glogger-personal.key{,.password}
# and is mirrored into the fork's PERSONAL_TAURI_SIGNING_PRIVATE_KEY* secrets.
set -euo pipefail
cd "$(dirname "$0")/.."

CONFIGS=(--config src-tauri/tauri.personal.conf.json)
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
  echo "note: TAURI_SIGNING_PRIVATE_KEY unset -> building without updater artifacts (local install only)"
  CONFIGS+=(--config '{"bundle":{"createUpdaterArtifacts":false}}')
fi

exec node node_modules/@tauri-apps/cli/tauri.js build "${CONFIGS[@]}"
