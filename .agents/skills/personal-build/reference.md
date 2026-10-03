# glogger personal build — integration & release pipeline (fork-local)

Skill for managing the **`local/integration`** branch: merging work in,
building the `glogger.Personal` daily driver, and keeping data intact.
Everything here is fork-local by design — tracked on `local/integration`
but never PR'd to `crisp-oddio/glogger-oddio`, and `local/*` branches
are never pushed to the fork.

## Branch map

| Branch | Purpose | Push? |
|---|---|---|
| `main` | Mirror of `origin/main` (this fork's shared branch). Pin to `origin/main`. | only upstreamable fixes |
| `local/integration` | The daily-driver source of truth: personal fixes + cherry-picked work. | NO |
| `fix/*`, `feat/*` | Reviewable single-purpose branches cut for eventual upstream PRs. | YES (to fork) |
| `upstream/*` | `crisp-oddio` remotes — read-only reference. | never |

## Key identifiers & profiles

| Config file | Identifier | Appdata dir | Use |
|---|---|---|---|
| `src-tauri/tauri.release.conf.json` | `glogger.Release` | `%APPDATA%\glogger.Release` | **Production install — never overwrite casually** |
| `src-tauri/tauri.personal.conf.json` | `glogger.Personal` | `%APPDATA%\glogger.Personal` | **Daily driver from this branch** |
| `src-tauri/tauri.experimental.conf.json` | `glogger.Experimental` | `%APPDATA%\glogger.Experimental` | Tester sandbox (auto-reseed on version bump — do NOT use for personal data) |
| `src-tauri/tauri.conf.json` (default) | `glogger.Dev` | `%APPDATA%\glogger.Dev` | `npm run tauri dev` scratch DB |

## Commands

```powershell
# Build the personal installer (from local/integration):
npm run tauri:build:personal
# → src-tauri/target/release/bundle/nsis/*.exe

# Quick dev-run of the integration branch (debug build):
npm run tauri dev            # glogger.Dev DB — testing only
npm run tauri:dev:experimental  # Experimental profile — auto-reseeds on version bump

# Verify before/after merges:
cd src-tauri && cargo test --lib && cargo check
# frontend type gate:
npm run build
```

## Routine: "I want fix X in my daily glogger"

1. **Source the fix.** If it lives on a side branch: `git merge fix/<name>` into
   `local/integration`. If it's uncommitted work: commit it to
   `local/integration` in logical chunks first (small commits, honest
   messages). If it's a TODO-style change made directly on
   `local/integration`: nothing to do.
2. **Verify:** `cd src-tauri && cargo test --lib` (expect 585+, 0 failed) and
   `npm run build` (vue-tsc) if frontend changed.
3. **Build:** `npm run tauri:build:personal` (~3–10 min; first build ~20 min).
4. **Install:** run the produced NSIS setup from
   `src-tauri/target/release/bundle/nsis/`. Installing over the previous
   personal build preserves `%APPDATA%\glogger.Personal` untouched.
5. **Smoke:** launch, check window title `glogger v<version> PERSONAL`,
   confirm live data (new events since the last session appear).

## Data safety invariants (NEVER break these)

- The Personal profile has **no auto-reseed**: data in
  `%APPDATA%\glogger.Personal` persists across rebuilds AND version bumps.
- **Never** point the production `glogger.Release` install at a dev/personal
  DB, and never copy DBs while a glogger is running (SQLite WAL — copy only
  when that source app is closed).
- Copying DBs: copy `glogger.db` + `glogger.db-wal` + `glogger.db-shm`
  together, source app closed. Verify with a count-diff on a couple of
  tables (`words_of_power`, `character_resuscitations`).
- `version:bump` only changes title strings for personal/experimental — no
  data impact for the Personal profile.
- Migrations run **upward only**. A DB that has touched vN keeps vN schema.
  Never open a newer-schema DB with an older glogger build.

## Windows/PowerShell gotchas (this machine)

- PowerShell 5.1 mangles `git commit -m` here-strings with double quotes —
  use `git commit -F <file>`.
- `src-tauri/Cargo.toml` shows a phantom CRLF-only diff in `git status` —
  leave it uncommitted (permanent known artifact).
- npm only (Node 24); no bun/yarn/pnpm.
- Commit prefixes: `feat:` / `fix:` / `impv:` / `docs:` / `test:` / `build:`.

## Upstream PR path (later)

Per `four-pr-plan.md` (repo root, untracked): upstream PRs are cut from
`upstream/dev` (crisp-oddio), NOT from `local/integration` or `main`.
Cherry-pick the relevant commits onto a fresh branch off `upstream/dev`,
rebase if the tip moved, `gh pr create --repo crisp-oddio/glogger-oddio
--base dev`. The local pre-push constraint scanner
(`scripts/check-glogger-constraints.ps1`) runs automatically on push.
