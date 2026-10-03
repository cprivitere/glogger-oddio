---
name: personal-build
description: Operate the glogger personal daily-driver pipeline — integrate tested work into the local/integration branch, build/install the glogger.Personal app (npm run tauri:build:personal), keep %APPDATA%\glogger.Personal data safe, and never PR personal-only files upstream.
---

# Glogger Personal Build — Daily Driver Pipeline

## Overview
Operate the user's personal glogger daily-driver: integrate tested work into the `local/integration` branch, build/install the `glogger.Personal` app, and keep production data safe. Fork-local workflow; NEVER push `local/*` branches or PR personal-only files upstream.

## Quick reference

| Task | Command |
|---|---|
| Integrate tested work | `git checkout local/integration` → `git merge <branch-or-commit>` (or commit logical chunks) |
| Verify before build | `cd src-tauri && cargo test --lib` (expect 585+, 0 failed); `npm run build` if frontend touched |
| Build daily driver | `npm run tauri:build:personal` (from repo root) |
| Installer output | `src-tauri/target/release/bundle/nsis/*.exe` |
| Install/update | Run the NSIS setup — appdata `%APPDATA%\glogger.Personal` is preserved on reinstall |
| Fresh production data copy | Close glogger → copy `%APPDATA%\glogger.Release\{glogger.db,glogger.db-wal,glogger.db-shm,settings.json}` → `%APPDATA%\glogger.Personal\` |
| Type check only | `npm run build` (vue-tsc) |

## Decision rules

- **Which profile is the user running?** Window title: `glogger v<ver> PERSONAL` = daily driver (appdata `glogger.Personal`, data persists forever). `EXPERIMENTAL` = sandbox, auto-reseeds (wipes) on version bump — never treat its data as valuable. Plain `glogger` (installed) = production Release (appdata `glogger.Release`) — do not overwrite its DB with test data.
- **Where does new work live?** Committed work goes on `local/integration` in logical chunks (commit prefixes: `feat:`/`fix:`/`impv:`/`docs:`/`test:`/`build:`). Upstreamable fixes may also land on reviewable side branches (`fix/*`, `feat/*`) for later cherry-picking per `four-pr-plan.md` (cut from `upstream/dev`, target `crisp-oddio/glogger-oddio` base `dev`).
- **Never PR to upstream:** personal-only files — `AGENTS.md`, `four-pr-plan.md`, `.agents/skills/personal-build/`, `src-tauri/tauri.personal.conf.json` — and anything referencing `glogger.Personal` or personal data. All are committed to `local/integration` and excluded from upstream PR branches purely at cherry-pick time (commit-level exclusion).
- **DB safety:** copy DBs only when the source app is closed; copy `glogger.db` + `-wal` + `-shm` together; verify with count-diff (`words_of_power`, `character_resuscitations`). Migrations are upward-only — never open a newer-schema DB with an older build.

## Build pipeline details

1. Pre-flight: confirm branch = `local/integration`, tree clean (`git status --short`).
2. Verify: `cargo test --lib` (src-tauri dir) + `npm run build` if TS changed.
3. Build: `npm run tauri:build:personal`. NSIS bundle lands in `src-tauri/target/release/bundle/nsis/`.
4. Install: run the setup exe (installs to `%LOCALAPPDATA%\glogger-personal`, overwrites previous personal build, does not touch data dir).
5. Smoke: launch → title `glogger v<ver> PERSONAL` → confirm recent events (log positions advance).

## Data recovery / resync

If the Personal DB drifts stale and the user wants a fresh production snapshot: close ALL gloggers, copy the four files from `glogger.Release` to `glogger.Personal` (overwrite), relaunch. Note migrations run on the copy at launch (upward-only; the copy adopts the newer schema permanently).

## Known hazards (this machine, Windows)

- PowerShell 5.1 mangles `git commit -m` here-strings with double quotes → `git commit -F <file>`.
- `src-tauri/Cargo.toml` phantom CRLF diff in `git status` — leave uncommitted.
- npm only (Node 24). No bun/yarn/pnpm.
- Do not run two glogger instances against the same data dir simultaneously.
- `version:bump` does NOT wipe Personal data (no seed gate on `glogger.Personal`); it DOES wipe Experimental data — that's by design.
- Updater signing: `tauri.conf.json` has `createUpdaterArtifacts: true`, so builds error at the signing step unless `TAURI_SIGNING_PRIVATE_KEY` is set. The personal profile already overrides it to `false` (`tauri.personal.conf.json`) — if that override ever disappears, the error is benign (installer is already produced).

## Deep reference

Branch map, seed-gate internals, and upstream PR cutting details: read `skill://personal-build/reference.md` (same directory as this file).
