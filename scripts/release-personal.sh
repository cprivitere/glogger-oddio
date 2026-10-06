#!/usr/bin/env bash
# Publish a personal build to the fork: bump version, commit, tag, push.
# CI (.github/workflows/personal-release.yml) builds + publishes the Release.
# Usage: scripts/release-personal.sh [patch|minor|major|x.y.z]
set -euo pipefail
cd "$(dirname "$0")/.."

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [ "$BRANCH" != "personal" ]; then
  echo "Error: must be on the 'personal' branch (currently '$BRANCH')"; exit 1
fi
if [ -n "$(git status --porcelain)" ]; then
  echo "Error: working tree not clean"; git status --short; exit 1
fi

scripts/bump-version.sh "${1:-patch}"
VERSION="$(sed -n 's/.*"version"\s*:\s*"\([0-9]*\.[0-9]*\.[0-9]*\)".*/\1/p' src-tauri/tauri.conf.json | head -1)"
TAG="v${VERSION}-personal"

git add -A
git commit -m "release: ${TAG}"
git tag "$TAG"
git push origin personal
git push origin "$TAG"
echo "Pushed ${TAG} — the Personal Release workflow will build and publish it."
