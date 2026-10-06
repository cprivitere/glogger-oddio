# glogger personal build — integration & release pipeline (fork-local)

Skill for managing the **`personal`** branch: merging work in,
building the `glogger.Personal` daily driver, and keeping data intact.
Everything here is fork-local by design — tracked on `personal`
but never PR'd to `crisp-oddio/glogger-oddio`. The `personal` branch IS
pushed to the fork (`origin`) — it is the source of truth for the fork's
`v<version>-personal` Releases (`.github/workflows/personal-release.yml`).

## Branch map

| Branch | Purpose | Push? |
|---|---|---|
| `main` | Mirror of `origin/main` (this fork's shared branch). Pin to `origin/main`. | only upstreamable fixes |
| `personal` | The daily-driver + fork-release source of truth: personal fixes + cherry-picked work. | YES (to `origin`) |
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
# Build the personal installer (from personal):
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
   `personal`. If it's uncommitted work: commit it to
   `personal` in logical chunks first (small commits, honest
   messages). If it's a TODO-style change made directly on
   `personal`: nothing to do.
2. **Verify:** `cd src-tauri && cargo test --lib` (expect 585+, 0 failed) and
   `npm run build` (vue-tsc) if frontend changed.
3. **Build:** `npm run tauri:build:personal` (~3–10 min; first build ~20 min).
4. **Install:** run the produced NSIS setup from
   `src-tauri/target/release/bundle/nsis/`. Installing over the previous
   personal build preserves `%APPDATA%\glogger.Personal` untouched.
5. **Smoke:** launch, check window title `glogger v<version> PERSONAL`,
   confirm live data (new events since the last session appear).

## Routine: publish a fork Release

```bash
npm run release:personal <patch|minor|major|x.y.z>
```

Must be run on `personal` with a clean tree. It runs `scripts/bump-version.sh`
(sets `0.12.18`-style version everywhere, including the personal window
title), commits `release: v<version>-personal`, tags it, and pushes both the
branch and the tag to `origin`. CI
(`.github/workflows/personal-release.yml`) then builds the **signed** Windows
NSIS `glogger.Personal` installer and publishes it to
`https://github.com/cprivitere/glogger-oddio/releases/tag/v<version>-personal`
as `glogger-<version>-personal-windows-setup.exe` (+ `.sig`), and refreshes the
update channel (next section). The upstream Flatpak workflow ignores
`v*-personal` tags.

## Auto-update & signing

The `glogger.Personal` profile ships a working in-app updater
(`tauri-plugin-updater`, polled 5 s after startup + hourly; banner in the
header, full UI in Help → Changelog).

| Piece | Value |
|---|---|
| Private key (build-time signing) | `C:\Users\cprivitere\.tauri\glogger-personal.key` (password in `glogger-personal.key.password` next to it) — **outside the repo; this is the only copy** |
| Fork secrets | `PERSONAL_TAURI_SIGNING_PRIVATE_KEY`, `PERSONAL_TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (set via `gh secret set … --repo cprivitere/glogger-oddio --body "$(cat <file>)"`) |
| Public key (compiled into the app) | `plugins.updater.pubkey` in `src-tauri/tauri.personal.conf.json` |
| Update endpoint (compiled into the app) | `https://github.com/cprivitere/glogger-oddio/releases/download/personal-latest/latest.json` |
| Manifest producer | `personal-release.yml` → *Generate latest.json updater manifest* → uploaded to the `personal-latest` pointer release (`--clobber`) |
| Manually installed `.sig` verification | `tauri signer sign` produces the same minisign payload; CI verifies `latest.json` parses |

Why a `personal-latest` pointer release instead of `releases/latest/download/…`:
`--latest` on the fork is shared with the fork's `glogger.Release` (upstream-mirror)
releases, which are signed with a *different* key. A dedicated tag keeps the
personal channel pointing only at personal builds — and its URL is stable, so
the endpoint never has to change.

Invariants (break either one and updates fail silently — `check()` still
reports an update, the install fails at signature verification):

1. `tauri.personal.conf.json`'s `pubkey` == public half of the secret.
2. The endpoint stays the `personal-latest` tag.

Local builds are **unsigned** by design (`scripts/personal-build.sh` adds a
`createUpdaterArtifacts:false` override when no key is in the environment) —
that is harmless, because the updater verifies the artifact it *downloads*, not
the running binary. For a signed local build:

```bash
TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/glogger-personal.key)" \
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat ~/.tauri/glogger-personal.key.password)" \
npm run tauri:build:personal
```

Rotating the key: `npx tauri signer generate -w ~/.tauri/glogger-personal.key -p '<pw>' -f`
(via `node node_modules/@tauri-apps/cli/tauri.js signer generate …` — bare `npx tauri` fails on this machine), update both secrets, then paste the new
`glogger-personal.key.pub` content into `plugins.updater.pubkey` and publish a
release. **Installed builds cannot auto-update across a key rotation** — the old
pubkey is compiled in, so the new-key release must be installed manually once.

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
`upstream/dev` (crisp-oddio), NOT from `personal` or `main`.
Cherry-pick the relevant commits onto a fresh branch off `upstream/dev`,
rebase if the tip moved, `gh pr create --repo crisp-oddio/glogger-oddio
--base dev`. The local pre-push constraint scanner
(`scripts/check-glogger-constraints.ps1`) runs automatically on push.
