# Deferred Tuning — `feat/books-watcher`

Items found during the 2026-10-04 review round (commit `d1533ec` and
predecessors) that are deliberate, documented deferrals: each one needs
real-world data or a subjective call before it can be tuned. Work them
one at a time while using the app day-to-day.

Related plan files: none yet — promote an item to its own
`docs/plans/*.md` if it grows beyond a checklist.

---

## 1. Rez dedup 30-second window — needs real-log calibration

**Where:** `src-tauri/src/coordinator.rs` — `rez_within_dedup_window`
(`WINDOW = 30s`), guard at the `parse_resuscitate_message` dispatch
(~L1500), tests in `mod rez_dedup_tests`.

**Why deferred:** The guard suppresses the second phrasing of one rez
("X resuscitates Y" → "Y comes back to life!") only if the pair is ≤30s
apart and forward-ordered. Both assumptions are unverified against real
game behavior — tuning requires a capture of actual paired lines.

**Concrete open items:**
- [ ] Capture a real session containing at least one rez pair; measure
      the actual timestamp gap between the two phrasings. If any pair
      exceeds 30s, widen `WINDOW` (there's no re-rez-cooldown evidence
      either way yet, so the ceiling is unknown).
- [ ] Backward-ordered pairs fail OPEN (both rows persist): if the
      paired line's timestamp is *earlier* than the first persisted
      event, `rez_within_dedup_window` returns false (guard is
      forward-only, documented as deliberate in the code + test
      `distinct_events_and_malformed`). Fix options: make the window
      bidirectional (`|cur - prev| <= WINDOW`), or first prove chat-log
      timestamps are monotonic within a file and document that as an
      invariant instead.
- [ ] Symptom to watch for: duplicate rez rows / inflated "times rezzed"
      counts in the death-tracker widget. `character_resuscitations` has
      no UNIQUE key, so dupes are silent — cosmetic only, no corruption.
- [ ] Same-minute-bucket behavior intentionally gone: the old
      `(target, minute)` scheme suppressed pairs up to ~60s apart and
      straddling minute boundaries were MISSED; the new rolling window
      fixes straddles but shortens coverage. If capture shows gaps of
      30–60s, consider a 60s window as the old-equivalent compromise.

**How to verify after changing:** `cargo test --lib rez_dedup_tests`
(adjust window-boundary assertions if `WINDOW` changes).

---

## 2. Brewing bulk scan — memory vs. snapshot volume

**Where:** `src-tauri/src/db/brewing_commands.rs` — Phase 1 of
`scan_all_snapshots_for_brewing` materializes every snapshot's full
`raw_json` + all parsed `BrewingItem` vecs simultaneously (previously
one snapshot at a time). The all-or-nothing Phase 3 transaction is an
improvement and should be kept regardless.

**Why deferred:** Memory scales with total snapshot JSON size. Whether
that matters depends on real snapshot counts/sizes on the user's
machine — pure tuning, no correctness bug.

**Concrete open items:**
- [ ] Measure real-world volume: snapshot row count × average
      `raw_json` size (e.g. `SELECT COUNT(*), AVG(LENGTH(raw_json)),
      SUM(LENGTH(raw_json)) FROM character_item_snapshots`). If the sum
      stays in the low tens of MB, leave as-is and note it here.
- [ ] If it grows past that: batch the scan (transaction per ~50
      snapshots, parsing inside each batch window). Keep the
      error-all-or-nothing semantics per batch, and keep Phase 1's
      read-connection scoping (read conns dropped before any `await`
      — that pattern was verified safe; don't regress it).
- [ ] TOCTOU between read/write phases is nominal (values captured
      once, snapshot rows are write-once) — no action needed, noted
      here so nobody "fixes" it into a re-read later.

---

## 3. ChatMessageList activeDay scroll reset — nextTick race

**Where:** `src/components/Chat/ChatMessageList.vue` — the `watch` on
`props.dateNav?.activeDay` (~L330) that resets `scrollTop = 0` after a
day jump / "Back to Live".

**Why deferred:** The watcher awaits a single `nextTick()` before
resetting, but the parent (ChatSearchView) assigns `messages` only
after its own `await invoke(...)` resolves. If the parent's assignment
lands *after* this watcher's nextTick fires, the reset hits the OLD
scroll height and the newly rendered list can re-clamp `scrollTop` to
non-zero — day-jump would land mid-list. Intermittent by nature; not
statically provable. Needs live observation.

**Concrete open items:**
- [ ] Repro attempt: slow network/dev-tools throttling + jump to a day
      far from the current window; check whether the list sometimes
      opens mid-list instead of at the top. Desc AND asc sort orders
      (top of window is the anchor either way — verified).
- [ ] If it reproduces: preferred fix is watching both sources —
      `watch([() => props.dateNav?.activeDay.value, () => props.messages],
      ...)` so the reset runs only after BOTH change; alternative is
      double `nextTick`.
- [ ] Known-good today (verified in review): no live conflict with the
      `messages.length`/`highlightId` watchers (no view passes
      `auto-scroll`; context mode clears highlightId before the day
      jump). If a future view passes both `auto-scroll` and `date-nav`,
      re-check ordering here.
- [ ] Do NOT add a scroll fight: any fix must keep the
      "scroll to highlight" watcher winning when `highlightId` is set.
