---
name: personal-build
description: Operate the glogger personal daily-driver pipeline — integrate tested work into the personal branch, build/install the glogger.Personal app (npm run tauri:build:personal), keep %APPDATA%\glogger.Personal data safe, and never PR personal-only files upstream.
---

# Glogger Personal Build — Daily Driver Pipeline

## Overview
Operate the user's personal glogger daily-driver: integrate tested work into the `personal` branch, build/install the `glogger.Personal` app, and keep production data safe. The `personal` branch IS pushed to `origin` (it is the fork-release source of truth); NEVER PR personal-only files upstream.

## Quick reference

| Task | Command |
|---|---|
| Integrate tested work | `git checkout personal` → `git merge <branch-or-commit>` (or commit logical chunks) |
| Verify before build | `cd src-tauri && cargo test --lib` (expect 585+, 0 failed); `npm run build` if frontend touched |
| Build daily driver | `npm run tauri:build:personal` (from repo root; unsigned locally — see *Auto-update*) |
| Installer output | `src-tauri/target/release/bundle/nsis/*.exe` |
| Publish a fork release | `npm run release:personal <patch\|minor\|major\|x.y.z>` (bumps, commits, tags `v<version>-personal`, pushes; CI builds + signs + publishes the fork Release and refreshes the update channel) |
| Install/update | Run the NSIS setup — appdata `%APPDATA%\glogger.Personal` is preserved on reinstall. Once a signed build is installed, the app updates itself (banner in the header / Help → Changelog) |
| Signing key material | `~/.tauri/glogger-personal.key` + `.password` (outside the repo) ↔ fork secrets `PERSONAL_TAURI_SIGNING_PRIVATE_KEY[_PASSWORD]`; pubkey embedded in `src-tauri/tauri.personal.conf.json` |
| Fresh production data copy | Close glogger → copy `%APPDATA%\glogger.Release\{glogger.db,glogger.db-wal,glogger.db-shm,settings.json}` → `%APPDATA%\glogger.Personal\` |
| Type check only | `npm run build` (vue-tsc) |

## Decision rules

- **Which profile is the user running?** Window title: `glogger v<ver> PERSONAL` = daily driver (appdata `glogger.Personal`, data persists forever). `EXPERIMENTAL` = sandbox, auto-reseeds (wipes) on version bump — never treat its data as valuable. Plain `glogger` (installed) = production Release (appdata `glogger.Release`) — do not overwrite its DB with test data.
- **Where does new work live?** Committed work goes on `personal` in logical chunks (commit prefixes: `feat:`/`fix:`/`impv:`/`docs:`/`test:`/`build:`). Upstreamable fixes may also land on reviewable side branches (`fix/*`, `feat/*`) for later cherry-picking per `four-pr-plan.md` (cut from `upstream/dev`, target `crisp-oddio/glogger-oddio` base `dev`).
- **Never PR to upstream:** personal-only files — `AGENTS.md`, `four-pr-plan.md`, `.agents/skills/personal-build/`, `src-tauri/tauri.personal.conf.json` — and anything referencing `glogger.Personal` or personal data. All are committed to `personal` and excluded from upstream PR branches purely at cherry-pick time (commit-level exclusion).
- **DB safety:** copy DBs only when the source app is closed; copy `glogger.db` + `-wal` + `-shm` together; verify with count-diff (`words_of_power`, `character_resuscitations`). Migrations are upward-only — never open a newer-schema DB with an older build.

## Build pipeline details

1. Pre-flight: confirm branch = `personal`, tree clean (`git status --short`).
2. Verify: `cargo test --lib` (src-tauri dir) + `npm run build` if TS changed.
3. Build: `npm run tauri:build:personal`. NSIS bundle lands in `src-tauri/target/release/bundle/nsis/`.
4. Install: run the setup exe (installs to `%LOCALAPPDATA%\glogger\`, overwrites previous personal build, does not touch data dir).
5. Smoke: launch → title `glogger v<ver> PERSONAL` → confirm recent events (log positions advance).

## Data recovery / resync

If the Personal DB drifts stale and the user wants a fresh production snapshot: close ALL gloggers, copy the four files from `glogger.Release` to `glogger.Personal` (overwrite), relaunch. Note migrations run on the copy at launch (upward-only; the copy adopts the newer schema permanently).

## Known hazards (this machine, Windows)

- PowerShell 5.1 mangles `git commit -m` here-strings with double quotes → `git commit -F <file>`.
- `src-tauri/Cargo.toml` phantom CRLF diff in `git status` — leave uncommitted.
- npm only (Node 24). No bun/yarn/pnpm.
- Do not run two glogger instances against the same data dir simultaneously.
- `version:bump` does NOT wipe Personal data (no seed gate on `glogger.Personal`); it DOES wipe Experimental data — that's by design.
- Updater signing (`createUpdaterArtifacts: true` on the personal profile): a build **fails** without `TAURI_SIGNING_PRIVATE_KEY`, but `npm run tauri:build:personal` runs `scripts/personal-build.sh`, which injects a `createUpdaterArtifacts:false` override when the variable is unset — so local builds stay unsigned and working. Never publish an unsigned build: CI's *Collect installer* step hard-fails when the NSIS `.exe.sig` is missing.
- **Auto-update correctness:** the `pubkey` in `src-tauri/tauri.personal.conf.json` must be the public half of the key in the fork's `PERSONAL_TAURI_SIGNING_PRIVATE_KEY` secret, and the endpoint must stay the `personal-latest` channel tag. If either drifts, every update is silently rejected (`check()` succeeds, install fails verification) — see *Auto-update & signing* in `reference.md`.

## Deep reference

Branch map, seed-gate internals, and upstream PR cutting details: read `skill://personal-build/reference.md` (same directory as this file).
