#!/usr/bin/env bash
# Publish a release: bump version, commit, tag v<version>, push.
# CI (.github/workflows/release.yml) builds, signs and publishes the Release.
# Usage: scripts/release.sh [patch|minor|major|x.y.z]
set -euo pipefail
cd "$(dirname "$0")/.."

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [ "$BRANCH" != "main" ]; then
  echo "Error: must be on the 'main' branch (currently '$BRANCH')"; exit 1
fi
if [ -n "$(git status --porcelain)" ]; then
  echo "Error: working tree not clean"; git status --short; exit 1
fi

scripts/bump-version.sh "${1:-patch}"
VERSION="$(sed -n 's/.*"version"\s*:\s*"\([0-9]*\.[0-9]*\.[0-9]*\)".*/\1/p' src-tauri/tauri.conf.json | head -1)"
TAG="v${VERSION}"

git add -A
git commit -m "release: ${TAG}"
git tag "$TAG"
git push origin main
git push origin "$TAG"
echo "Pushed ${TAG} — the Release workflow will build and publish it."
