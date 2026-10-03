# Auto-Capture Gaps — Four Upstream PRs

## Context

An audit of glogger's ingestion found data glogger already receives but never
auto-processes. Four independent fixes, each shipped as a separate PR from this
fork (`origin` = cprivitere/glogger-oddio) to the upstream repo
(`crisp-oddio/glogger-oddio`). All four branch from `upstream/dev` at `73823f2`
(verified ancestor of local `main`; local-only commits — stall-price v68,
compliance gate — are NOT prerequisites and MUST NOT be included). PRs target
`base: dev`, authored as the user's identity, created via
`gh pr create --repo crisp-oddio/glogger-oddio --base dev`.

End state:
1. **PR1 — Books-dir watcher**: the game's `Books/` folder of report files
   (SkillReport, PlayerAge, HelpScreen, PlayerShopLog, GuildMotd, MessageOfTheDay)
   is ingested like the live `ProcessBook` stream, so reports run while glogger
   was closed are captured on next launch.
2. **PR2 — Cook's Helper auto-import**: the Cook's Helper tab auto-imports the
   latest gourmand report on mount (mirroring Gourmand's `tryAutoImport`), and
   its file-picker defaults to the correct directory (`Books/`, not `Reports/`).
3. **PR3 — Notepad viewer**: the in-game NOTEPAD (all tabs), `HUNTING_GROUP_TITLE`,
   `FRIEND_STATUS`, `PUBLIC_STATUS` strings — already persisted by V33 to
   `game_state_strings` — get a getter command and a UI panel.
4. **PR4 — Quest history**: `ProcessAddQuest`/`ProcessCompleteQuest` log lines
   (never parsed) are ingested into a new `character_quest_history` table with
   CDN-resolved quest names, shown in the Character → Quests tab.

Repo conventions (AGENTS.md): additive migrations only (`migrations.rs` is at
V67 upstream); commands registered in `lib.rs` `generate_handler!` + `use` block;
Pinia stores need `acceptHMRUpdate` blocks; commit prefixes `feat:`/`fix:`;
`Result<T, String>` error style; snake_case commands.

---

## PR1 — `feat(books): ingest the game's Books directory on startup`

The game writes one .txt file per opened book into `<game_data_path>/Books/`
(verified live: `SkillReport_YYMMDD_HHMMSS.txt`, `PlayerAge_*.txt`,
`HelpScreen_*.txt`, `PlayerShopLog_*.txt`, `GuildMotd_*.txt`,
`MessageOfTheDay_*.txt`, `NewQuestquest_*.txt`). Today these are only captured
when `ProcessBook` streams through a running coordinator
(`coordinator.rs:782-857`); files written while glogger is closed sit on disk
unimported. Player-prev.log has the same gap and is already solved by
`replay::spawn_player_prev_watcher` (`replay.rs:847`) — this PR mirrors that
pattern for the Books dir.

### Approach

1. **New module `src-tauri/src/books_watcher.rs`** — no equivalent exists.
   `pub fn spawn_books_watcher(settings: std::sync::Arc<SettingsManager>,
   db: DbPool, app: AppHandle)` spawning a `std::thread` that:
   - Sleeps 15s before first scan (let startup settle — mirrors
     `spawn_player_prev_watcher`'s 10s sleep).
   - Loops every 30s: reads `last_mtime` of every `Books/*.txt` file; on
     any change (or first scan), processes all files once (per-file mtime
     tracking in a `HashMap<PathBuf, SystemTime>`; a file is re-processed
     only when its mtime changed).
   - No setting gate: books ingestion is the same privilege class as the
     live ProcessBook path (reading files the user's own client wrote to the
     user's own disk — limits doc §3). It respects empty `game_data_path`
     (skip silently, same as `import_latest_gourmand_report`).
   - Every 30s tick, also re-check `settings.get_game_data_path()` so a
     changed path takes effect without restart (pattern:
     `get_auto_ingest_player_prev()` re-check in `spawn_player_prev_watcher`).

2. **Per-file dispatch** (a free function
   `fn process_book_file(path: &Path, content: &str, character: &str,
   server: &str, db: &DbPool, ops_lock: &StallOpsLock, app: &AppHandle)
   -> Result<(), String>`), dispatching on filename prefix — the same rules
   the live coordinator applies to `BookOpened`:
   - `SkillReport_*.txt`:
     - If `content.trim_start().starts_with("Foods Consumed:")` →
       `db::gourmand_commands::import_gourmand_from_content(&conn, content)`
       (exists, idempotent upsert by food_name; preserves manual marks) and
       on `Ok(n > 0)` emit `gourmand-updated` with `n` (matches
       `coordinator.rs:803-827`).
     - If `content.trim_start().starts_with("Equipment Studied:")` and
       `content.contains("Primary Bind Location:")` → both
       `ingest_hoplology_report`- and `ingest_teleportation_binds`-equivalent
       handling. These are coordinator methods using
       `self.game_data`/`self.active_character_server()`; **extract** their
       bodies into free functions in `coordinator.rs`:
       `pub fn ingest_hoplology_content(conn: &Connection, character: &str,
       server: &str, content: &str) -> Result<usize, String>` and
       `pub fn ingest_teleport_binds(conn: &Connection, character: &str,
       server: &str, content: &str) -> Result<usize, String>`, then make the
       coordinator methods thin wrappers (same behavior, same emits). If
       extraction of `ingest_hoplology_report` turns out to require more of
       the coordinator than `(conn, character, server)` — unverified, read
       `coordinator.rs:2401` first — extract only what compiles cleanly and
       document the residual coupling in the PR description instead of
       duplicating logic.
     - Otherwise (generic SkillReport) → book-content upsert (below).
   - `PlayerAge_*.txt` → book upsert + `report_stats::parse_player_age` +
     `report_stats::persist_stats` (both exist; upsert by
     character/server/category/stat_name — re-import safe).
   - `HelpScreen_*.txt` → book upsert + `parse_behavior_report` +
     `persist_stats`.
   - `PlayerShopLog_*.txt` → mirror `ingest_shop_log` (`coordinator.rs:2619`):
     `parse_shop_log("Imported", content, "imported", base_year)` probe →
     `stall_year_resolver::resolve_timestamps_oldest_first` year resolution —
     NOTE: the file variant must use the **file-export path already present**:
     `db::stall_tracker_commands` has no public file-import helper that
     resolves years; reuse `year_from_filename` logic by calling
     `parse_shop_log(title, content, "imported", year_from_filename(path))`
     (the exact pairing used by `import_shop_log_file`,
     `stall_tracker_commands.rs:865-880`). Make `year_from_filename` `pub(crate)`
     (currently private at `stall_tracker_commands.rs:810`). Insert via
     `insert_stall_events(&db, &ops_lock, &inputs)` with
     `owner: Some(character)` (mirrors live ingest), then emit
     `stall-events-updated` with the inserted count on `Ok(n > 0)`.
   - `GuildMotd_*.txt` / `MessageOfTheDay_*.txt` / `NewQuestquest_*.txt` →
     book upsert only (no structured parser exists; capture the content so
     it is queryable).
   - Unknown prefix → book upsert only.

3. **Book upsert helper** — the `persist_book_report` body
   (`coordinator.rs:2029-2057`) is a coordinator method but only uses
   `(db_pool, app_handle)`; extract to a free function
   `pub fn persist_book_content(db: &DbPool, app: &AppHandle, character: &str,
   server: &str, book_type: &str, title: &str, content: &str) -> Result<(), String>`
   with the identical upsert
   (`INSERT INTO game_state_books ... ON CONFLICT(character_name, server_name,
   book_type, title) DO UPDATE`) + `emit("game-state-updated", vec!["books"])`.
   Coordinator's `persist_book_report` becomes a wrapper around it (same
   behavior). Book type derives from the filename prefix: `SkillReport` →
   `"SkillReport"`, `PlayerAge` → `"PlayerAge"`, `HelpScreen` → `"HelpScreen"`,
   `PlayerShopLog` → `"PlayerShopLog"`, `GuildMotd` → `"GuildMotd"`,
   `MessageOfTheDay` → `"MessageOfTheDay"`, `NewQuest` → `"NewQuestBook"`;
   title = the file's basename.

4. **Character identity**: the watcher reads
   `settings.get().active_character_name` and
   `settings.get().active_server_name` (fields at `settings.rs:83,87`) each
   scan; if `active_character_name` is `None` or empty, skip the scan tick
   entirely (same guard as `ingest_shop_log`). This matches the live path's
   identity semantics — BookOpened events also attribute to the active
   character.

5. **Wire-up in `lib.rs`**: after the Step 5c
   `replay::spawn_player_prev_watcher(...)` call (`lib.rs:505`), add
   `books_watcher::spawn_books_watcher(settings_manager.clone(),
   db_pool.clone(), app_handle.clone());` under a new
   `// Step 5d+0: Books-dir backfill watcher` comment. `app_handle` is in
   scope there (it exists at Step 5c's call site — `ingest_shop_log` gets it
   via coordinator, but the watcher needs the handle passed directly;
   `spawn_player_prev_watcher` proves plain threads work here). Add
   `mod books_watcher;` next to `mod replay;` (~`lib.rs:12`).

6. **Tests** (`#[cfg(test)] mod tests` at file bottom of `books_watcher.rs`):
   - Fixture strings copied verbatim from the real files (first lines quoted
     in this plan's Context and verified this session):
     `process_book_file` dispatch table — each of the 7 prefixes routes to
     the right importer (assert via an in-memory `Connection` + behavior:
     foods count after gourmand import; report_stats rows after PlayerAge;
     stall_events row after PlayerShopLog sample line
     `Mon Aug 24 23:52 - GorgonoidNeurodivergente bought Mastery Spark: Staves at a cost of 10000 per 1 = 10000`).
   - `year_from_filename` publicity test is implicitly covered by the
     existing stall tracker tests once made `pub(crate)`.
   - Book upsert idempotency: same content twice → 1 row.

### Critical files & anchors
| File | Why |
|---|---|
| `src-tauri/src/books_watcher.rs` (new) | watcher thread + dispatch |
| `src-tauri/src/coordinator.rs:2029` `persist_book_report`; `:2401` `ingest_hoplology_report`; `:2344` `ingest_teleportation_binds` | extract to free fns; wrappers keep behavior |
| `src-tauri/src/db/stall_tracker_commands.rs:810` `year_from_filename` | make `pub(crate)` |
| `src-tauri/src/lib.rs:505` (Step 5c site), `:12` mod list, `:672` generate_handler | wire-up |
| `src-tauri/src/settings.rs:83,87` | identity fields read by watcher |

### Verification
1. `cd src-tauri && cargo test --lib` — all existing tests plus the new
   `books_watcher` tests pass (upstream CI runs this gate on ubuntu).
2. New-behavior check (manual, dev machine): with glogger CLOSED, run the
   Gourmand skill report in-game (creates a new `SkillReport_*.txt`), then
   start glogger; within ~45s of startup the Gourmand tab shows the new
   counts without visiting any other tab first (watcher runs on the backend,
   independent of `GourmandView` mount), and `startup_log!`/eprintln shows
   the import line. Reverse-check: delete `game_state_books` row for GuildMotd
   (or use a fresh character row) and confirm the file upserts on first scan.
3. `cargo check` clean; the pre-push constraint scanner passes (the watcher
   reads the user's own Books dir — an explicitly allowed category; no
   forbidden identifiers introduced).

---

## PR2 — `fix(crafting): Cook's Helper auto-imports the latest gourmand report`

`cooksHelperStore.ts` requires a manual file-picker import
(`importFile()`, `cooksHelperStore.ts:148-176`) even though the identical
report-parsing machinery (`parse_gourmand_report`,
`gourmand_commands.rs:423`) is already auto-wired for the Gourmand tab via
`import_latest_gourmand_report` (`gourmand_commands.rs:285`) — which dedups by
count-comparison and preserves manual marks. Also verified: the picker's
`defaultPath` is `gameDataPath + '/Reports'` (`cooksHelperStore.ts:152`) but
the game writes `SkillReport_*.txt` into `Books/` (verified on disk) — the
manual picker opens in the wrong directory today.

### Approach

1. **`cooksHelperStore.ts`** — add `async function tryAutoImport()` mirroring
   `gourmandStore.ts:294-304` verbatim in shape:
   ```ts
   async function tryAutoImport() {
     try {
       const result = await invoke<GourmandImportResult | null>('import_latest_gourmand_report')
       if (result) await loadFoodsAndRecipes()
     } catch (e) {
       console.warn('Cooks Helper auto-import:', e)
     }
   }
   ```
   Export it in the store's return object. No new Rust: the existing command
   is exactly the needed behavior (reads `Books/SkillReport_*` → parses →
   count-dedup → returns `null` when nothing new).
2. **`CooksHelperTab.vue`** — in the existing `onMounted` (~line 185), add
   `void store.tryAutoImport()` before `craftingStore.loadProjects()`.
   Importing twice is harmless (count-comparison skip). When auto-import
   yields data, the importedEatenNames UI shows it via the existing reactive
   flow — `importedEatenNames` is only set by manual import; auto-import does
   not set it (gourmand's tab shows imported data through the shared DB;
   cooks helper's eaten-list derives from `importedEatenNames`, so also call
   `store.refreshImportedNames()` if such a loader exists — unverified, read
   `cooksHelperStore.ts` for the consumed-foods derivation; if the eaten
   names are ONLY populated by manual import, set
   `importedEatenNames.value = new Set(names-from-DB)` by reusing whatever
   query exists, or fall back to calling the manual import's post-load path
   `loadFoodsAndRecipes()` which is already in scope — do not invent a new
   backend command for this).
3. **Fix the picker default** (`cooksHelperStore.ts:152`):
   `gameDataPath + '/Reports'` → `gameDataPath + '/Books'` (the actual
   location of `SkillReport_*.txt`), keeping the `txt` filter.
4. **Store HMR block** (AGENTS.md gotcha): `cooksHelperStore.ts` has NO
   `acceptHMRUpdate` today; add it at file bottom, pattern from
   `buildPlannerStore.ts`:
   ```ts
   if (import.meta.hot) {
     import.meta.hot.accept(acceptHMRUpdate(useCooksHelperStore, import.meta.hot))
   }
   ```
   importing `acceptHMRUpdate` from `pinia`.

### Critical files & anchors
| File | Why |
|---|---|
| `src/stores/cooksHelperStore.ts:148-176` | picker default + tryAutoImport + HMR block |
| `src/components/Crafting/CooksHelperTab.vue:185` | onMounted hook |
| `src-tauri/src/db/gourmand_commands.rs:285` | reused command (no change) |

### Verification
1. `npm run build` (vue-tsc + vite) clean.
2. New-behavior check: with a `SkillReport_*.txt` in `Books/` newer than the
   DB contents, open Craft Cook's Helper tab cold → the uneaten/project list
   reflects the report without clicking Import; reopen → no duplicate/extra
   work (count-dedup skips). Manual picker opens in `Books/`.

---

## PR3 — `feat(character): surface the in-game notepad (game_state_strings)`

V33 created `game_state_strings` (`migrations.rs:2514`, PK
`character_name, server_name, key`) and `game_state.rs:1044-1060` already
persists every `ProcessSetString` for the whitelisted keys `NOTEPAD`,
`NOTEPAD_TAB_1..4`, `NOTEPAD_TAB_NAMES`, `FRIEND_STATUS`, `PUBLIC_STATUS`,
`HUNTING_GROUP_TITLE` (`player_event_parser.rs:3260-3262`), emitting domain
`"strings"`. Nothing reads it: no getter exists
(`game_state_commands.rs:153-1616` has none) and `refreshDomain`
(`gameStateStore.ts:1084-1120`) has no `'strings'` case. The notepad the user
types into the game is captured and invisible.

### Approach

1. **Rust getter** `src-tauri/src/db/game_state_commands.rs` — new
   `#[tauri::command] pub fn get_game_state_strings(db: State<'_, DbPool>,
   character_name: String, server_name: String) -> Result<Vec<GameStateString>,
   String>`; struct:
   ```rust
   #[derive(Debug, Clone, serde::Serialize)]
   pub struct GameStateString {
       pub key: String,
       pub value: String,
       pub last_confirmed_at: String,
   }
   ```
   Query `SELECT key, value, last_confirmed_at FROM game_state_strings WHERE
   character_name = ?1 AND server_name = ?2 ORDER BY key` — direct mirror of
   `get_game_state_books` (`game_state_commands.rs:811-877`).
2. **Register** in `lib.rs`: add `get_game_state_strings` to the
   `db::game_state_commands::{...}` use-block (~`lib.rs:196-197`) and inside
   `generate_handler!`'s `// Game state queries` section (~`lib.rs:958-996`,
   next to `get_game_state_books`).
3. **Store wiring** `gameStateStore.ts`:
   - New state `const notepadStrings = ref<Record<string, string>>({})`
     (or a typed array; Record keyed by `key` is friendlier for the panel).
   - `refreshDomain` gains:
     ```ts
     case 'strings':
       notepadStrings.value = await invoke('get_game_state_strings', { characterName, serverName })
       break
     ```
   - Export `notepadStrings` in the store return.
4. **UI panel** `src/components/Character/NotepadPanel.vue` (new; pattern:
   `ReportStatsSection.vue` structure — header + loading/empty/else):
   - List the NOTEPAD tabs: `NOTEPAD_TAB_NAMES` holds the tab names
     (verified key whitelist); for each `NOTEPAD` / `NOTEPAD_TAB_1..4` key
     present, show a collapsible section titled with the tab name (fallback
     "Notepad") with the value rendered pre-formatted (`white-space:
     pre-wrap`, read-only — display only; editing stays in-game).
   - Show `HUNTING_GROUP_TITLE` as a small labeled line when present;
     `FRIEND_STATUS` / `PUBLIC_STATUS` likewise, each with its
     `last_confirmed_at` rendered via the same `formatTimestamp` helper
     ReportStatsSection uses.
   - Mount: in `CharacterView.vue` column 3
     (`CharacterView.vue:93-99` — the flex-col containing ComputedStatsCard +
     ReportStatsSection), add a third
     `<div class="bg-surface-elevated border border-border-default rounded-lg p-3 flex flex-col min-h-0 flex-1 overflow-hidden"><NotepadPanel /></div>`
     (the three share the column; acceptable for this PR — if the column gets
     cramped the reviewer may ask to move it to its own sub-tab; pre-decide:
     keep column 3, cap `max-h-[40%]` on the notepad wrapper).
   - `onMounted`: load via `invoke('get_game_state_strings', ...)` directly
     (pattern `ReportStatsSection.vue:137-161`) and subscribe
     `listen<string[]>('game-state-updated', e => { if
     (e.payload.includes('strings')) loadStrings() })`.

### Critical files & anchors
| File | Why |
|---|---|
| `src-tauri/src/db/game_state_commands.rs:803-877` | mirror this getter shape |
| `src-tauri/src/lib.rs:196,958-996` | use-block + registration |
| `src/stores/gameStateStore.ts:1084-1120` | refreshDomain `'strings'` case |
| `src/components/Character/NotepadPanel.vue` (new) | panel |
| `src/components/Character/CharacterView.vue:93-99` | mount point |

### Verification
1. `cargo test --lib` (upstream gate) + `npm run build` clean.
2. New-behavior check: type a line into the in-game notepad while glogger
   tails, then open Character → Stats: the line appears in the Notepad panel
   within ~1s (live event path emits domain `strings` → listener reloads).
   Restart persistence: the same line is still shown after app restart
   (DB-backed, PK'd per character/server).

---

## PR4 — `feat(quests): capture quest history from the log stream`

Quest lifecycle lines are in the log and never parsed (verified live):
```
[03:21:06] LocalPlayer: ProcessAddQuest(7556902, TransitionalQuestState)
[03:23:18] LocalPlayer: ProcessCompleteQuest(7556902, 25222)
```
`ProcessCompleteQuest(entityId, questId)` carries the numeric quest id; the
CDN `quests.json` keys are exactly `quest_<id>` (verified:
`quest_25222` → Name `"Hunting: Guard Scrays"`), and
`GameDataState.resolve_quest("quest_25222")` resolves it
(`game_data/mod.rs:453-461`; name at `quest.raw.get("Name")`).
CharacterSheet snapshots only show quests at snapshot time; there is no
longitudinal "when did I get/finish quest X" record.

### Approach

1. **Migration V68** (`migrations.rs`, after the `current_version < 67`
   block, upstream numbering — upstream/dev is at V67):
   ```rust
   fn migration_v68_quest_history(conn: &Connection) -> Result<()> {
       conn.execute_batch(
           "CREATE TABLE character_quest_history (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               character_name TEXT NOT NULL,
               server_name TEXT NOT NULL,
               quest_id TEXT NOT NULL,
               event TEXT NOT NULL,
               occurred_at TEXT NOT NULL,
               UNIQUE(character_name, server_name, quest_id, event, occurred_at)
           );
           CREATE INDEX idx_cqh_char ON character_quest_history(character_name, server_name, occurred_at DESC);
           CREATE INDEX idx_cqh_quest ON character_quest_history(quest_id);"
       )?;
       Ok(())
   }
   ```
   plus the `if current_version < 68 { migration_v68_quest_history(conn)?;
   super::record_migration(conn, 68)?; }` block after V67's, following the
   V23-template shape. `event` ∈ {`"added"`, `"completed"`}. `quest_id` is the
   string form of the numeric id (`"25222"`) — resolvable via
   `quest_<id>` key. NOTE for the implementer: the local fork uses V68 for
   `stall_price_observations` (unpushed). Whichever of the two lands upstream
   first owns V68; if this PR is rebased after that one merged, renumber to
   V69 (and so on) — additive-only, never edit a shipped migration.
2. **Parser** `player_event_parser.rs` — two new `PlayerEvent` variants next
   to the existing quest-less line handling (enum at ~`player_event_parser.rs:18`):
   ```rust
   QuestAdded { timestamp: String, quest_id: String },
   QuestCompleted { timestamp: String, quest_id: String },
   ```
   In `process_line` dispatch (~`line 1257`, the chain containing
   `ProcessUpdateQuest` currently unmatched) add before the generic tail:
   - `line.contains("ProcessAddQuest(")` → parse first arg as quest_id (u32
     stringified; second arg ignored),
   - `line.contains("ProcessCompleteQuest(")` → parse **second** arg
     (`ProcessCompleteQuest(6921655, 29601)` — the second is the quest id;
     verified format above).
   Parse with the existing arg-splitting helpers used by neighboring arms
   (`extract_quoted_string`-style numeric args — mirror
   `handle_process_complete_quest`-less neighbors; if a numeric-arg helper
   does not exist, a one-line `line` slice between parens split on `,`).
   Do NOT emit for `ProcessUpdateQuest`/`ProcessSelectQuest` (no id payload).
   Name resolution happens at the coordinator layer (game_data access), not
   in the parser.
3. **Coordinator** `game_state.rs` `process_events_batch` — new arms after
   `PlayerEvent::PlayerStringUpdated` (~`game_state.rs:1044`):
   - `QuestAdded` / `QuestCompleted` → INSERT
     `INSERT OR IGNORE INTO character_quest_history (character_name,
     server_name, quest_id, event, occurred_at) VALUES (?1..?5)` with
     `event` `"added"`/`"completed"`, `occurred_at = to_utc(timestamp)`;
     `domains.push("quests_history")`.
4. **Getter** `src-tauri/src/db/game_state_commands.rs`:
   ```rust
   #[derive(Debug, Clone, serde::Serialize)]
   pub struct QuestHistoryEntry {
       pub quest_id: String,
       pub quest_name: Option<String>,
       pub added_at: Option<String>,
       pub completed_at: Option<String>,
   }
   #[tauri::command]
   pub fn get_character_quest_history(db: State<'_, DbPool>, cdn:
   State<'_, GameDataState>, character_name: String, server_name: String)
   -> Result<Vec<QuestHistoryEntry>, String>
   ```
   One grouped query (MAX(CASE event)) over the table; resolve name via
   `cdn.read()` → `resolve_quest(&format!("quest_{}", quest_id))` →
   `quest.raw.get("Name").and_then(|n| n.as_str())` — same read pattern as
   `find_equipment_base_name` users in `coordinator.rs:1260-1266`
   (`self.game_data.try_read()`).
5. **Register** in `lib.rs` (use-block + `// Game state queries` section).
6. **Frontend**: Character → Quests tab. `src/components/Character/` has
   `character-quests.md` docs; find the quests tab component (read
   `CharacterView.vue` imports for the quests template branch ~line 109) and
   append a "History" section: a `QuestHistoryTable.vue` (new, minimal —
   name, added, completed columns; completed-first ordering; rows with
   `added_at IS NULL` show "—"). Store state in the existing quests-tab
   store/component (pattern: direct `invoke` in `onMounted` +
   `game-state-updated` listener filtered to `quests_history`, mirroring
   `ReportStatsSection.vue:157-161`). Load lazily on tab activation, not on
   app start.
7. **Tests**:
   - Parser: fixture lines verbatim from this session's Player.log
     (`[03:21:06] LocalPlayer: ProcessAddQuest(7556902, TransitionalQuestState)`,
     `[03:23:18] LocalPlayer: ProcessCompleteQuest(7556902, 25222)`) assert
     the right variant + id (`"25222"` for the Complete case, `"7556902"`
     is the ENTITY id — the Complete arm must pick the SECOND arg; include
     this exact line as the regression fixture).
   - DB: in-memory `run_migrations` + two inserts with same key → 1 row.

### Critical files & anchors
| File | Why |
|---|---|
| `src-tauri/src/db/migrations.rs:355-367` | V68 block after V67 |
| `src-tauri/src/player_event_parser.rs:18,1257` | enum + dispatch |
| `src-tauri/src/game_state.rs:1044-1060` | insert arms |
| `src-tauri/src/game_data/mod.rs:332,453` | `resolve_quest` for names |
| `src/components/Character/CharacterView.vue` (quests branch ~109) | tab mount |

### Verification
1. `cargo test --lib` + `npm run build` clean.
2. New-behavior check: complete any quest in-game with glogger tailing →
   Character → Quests → History shows the quest with a fresh `completed_at`
   within ~1s (live), name resolved from CDN (e.g. `quest_25222` renders
   "Hunting: Guard Scrays"). Restart → rows persist. Unknown id (CDN miss)
   renders the raw id — pre-decided fallback, not an error.

---

## Assumptions & contingencies

- **Base**: all four branches cut from `upstream/dev` (`73823f2`). If upstream
  `dev` moves before a PR opens, rebase the branch onto the new tip
  (`git fetch upstream && git rebase upstream/dev <branch>`) — all four touch
  disjoint files except `lib.rs`/`game_state_commands.rs` (PR3, PR4 both add
  entries there; rebase order = PR1 → PR2 → PR3 → PR4 keeps conflicts
  mechanical).
- **Migration numbering** (PR4 only): whichever of upstream-V68 lands first
  wins the number; the other rebases up. Pre-decided above.
- **PR1 hoplology extraction**: `ingest_hoplology_report`'
s coordinator
  coupling is unverified in full; fallback (extract to
  `(conn, character, server)` free fn; if it genuinely needs more
  coordinator state, ship the Books watcher with hoplology handling
  deferred and say so in the PR body) — never duplicate logic.
- **PR2 eaten-names derivation**: if `importedEatenNames` is manual-import
  only, the auto path reuses existing loaders per the step-2 note; no new
  backend command.
- **Commit/push mechanics** (all PRs): branch names `feat/books-watcher`,
  `fix/cooks-helper-auto-import`, `feat/notepad-panel`,
  `feat/quest-history`; push to `origin`; open with
  `gh pr create --repo crisp-oddio/glogger-oddio --base dev`. The local
  pre-push hook (`scripts/check-glogger-constraints.ps1`) runs on every push
  from this clone — all four PRs add no forbidden identifiers, so it stays
  green.
- **CI**: upstream's release workflow validates `npm run build` + `cargo
  check` + `cargo test` on ubuntu; PRs are gated on those. Node 24 + npm only
  (AGENTS.md toolchain rules).
