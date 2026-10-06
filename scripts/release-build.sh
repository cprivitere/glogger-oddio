#!/usr/bin/env bash
# Build the shipped glogger.Personal-free release flavour (identifier glogger.Release).
# bundle.createUpdaterArtifacts is true, so the CLI hard-fails without a signing
# key; a local installer never needs a signature (only the downloaded artifact is
# verified), so unless a key is already in the environment we turn updater
# artifacts off for this build.
set -euo pipefail
cd "$(dirname "$0")/.."

CONFIGS=(--config src-tauri/tauri.release.conf.json)
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
  echo "note: TAURI_SIGNING_PRIVATE_KEY unset -> building without updater artifacts (local install only)"
  CONFIGS+=(--config '{"bundle":{"createUpdaterArtifacts":false}}')
fi

exec node node_modules/@tauri-apps/cli/tauri.js build "${CONFIGS[@]}"
