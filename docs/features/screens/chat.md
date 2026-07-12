# Chat Logs Screen

## Overview

A full chat log browser that imports, indexes, and displays all in-game chat with full-text search, per-channel filtering, item link detection, and configurable watchword alerts. Chat data is parsed from the game's chat log files and stored in SQLite with FTS indexing.

The screen also hosts a **Poems** tab — a browsable, searchable collection of poems recited by other players at poetry podiums. Unlike the other tabs (which come from `Chat-*.log`), poems are sourced from `Player.log` `ProcessTalkScreen` events and stored globally (not per-character).

## Architecture

### Files

**Backend (Rust):**
- `src-tauri/src/chat_parser.rs` — chat log file parsing
- `src-tauri/src/chat_status_parser.rs` — status channel event parsing
- `src-tauri/src/chat_commands.rs` — Tauri command handlers
- `src-tauri/src/db/chat_commands.rs` — database query layer

**Frontend (Vue/TS):**
- `src/components/Chat/ChatView.vue` — 10-tab container
- `src/components/Chat/ChatMessageList.vue` — shared paginated message renderer
- `src/components/Chat/ChatSearchView.vue` — unified search with context navigation
- `src/components/Chat/ChatMessage.vue` — individual message display
- `src/components/Chat/MessageWithItemLinks.vue` — item link detection and rendering
- `src/utils/parseSearchQuery.ts` — search query parser for `from:`/`in:` operators
- Channel views: `ChannelView`, `TellsView`, `PartyView`, `NearbyView`, `GuildView`, `SystemView`, `AllMessagesView`, `WatchwordsView`
- `src/components/Chat/PoemsView.vue` — recorded-poems browser (list + full-poem viewer with prev/next)

**Poems pipeline (Rust):**
- `src-tauri/src/player_event_parser.rs` — `handle_talk_screen` detects `Poem by X` talk screens and emits `PlayerEvent::PoemRecorded` (`parse_poem_body` strips the intro/outro review blurbs and pulls out the bold title)
- `src-tauri/src/game_state.rs` — persists `PoemRecorded` into the `poems` table (`INSERT OR IGNORE` for dedup)
- `src-tauri/src/db/poem_commands.rs` — `get_poems` query command

**Stores:**
- `chatStore` — tailing state management
- `settingsStore` — watchword rule persistence

### Component Hierarchy

```
ChatView.vue                        — 9-tab container
├── ChatSearchView.vue              — unified search (default tab)
├── ChannelView.vue                 — public/custom channels with sidebar
├── TellsView.vue                   — direct messages with conversation list
├── PartyView.vue                   — party channel
├── NearbyView.vue                  — nearby/local chat
├── GuildView.vue                   — guild chat
├── SystemView.vue                  — system/status messages
├── AllMessagesView.vue             — global search across all channels
├── WatchwordsView.vue              — rule-based filtering and alerts
└── PoemsView.vue                   — recorded poems (self-contained; list + viewer)

Shared:
├── ChatMessageList.vue             — paginated message list (standard + bubble layouts)
└── MessageWithItemLinks.vue        — parses and links item references
```

## Per-Tab Documentation

- [chat-channels.md](chat/chat-channels.md) — Channels
- [chat-tells.md](chat/chat-tells.md) — Tells (Direct Messages)
- [chat-simple.md](chat/chat-simple.md) — Party, Nearby, Guild, System
- [chat-all.md](chat/chat-all.md) — All Messages (Global Search)
- [chat-watchwords.md](chat/chat-watchwords.md) — Watchwords (Alert Rules)

## Shared Components

### ChatMessageList

Generic message list renderer used by all tabs:
- **Standard layout** — timestamp, channel badge (optional), sender name, message body
- **Tell/bubble layout** — chat bubbles with player messages on right, others on left
- **Pagination** — infinite scroll (auto-loads at 200px from bottom), with fallback "Load More" button. 100 messages per page. Loading indicator is inline, preserving scroll position. Race-condition guarded (emit guard in scroll handler + loading check in parent).
- **Clickable messages** — optional `clickable` prop enables click-to-view-context; `highlightId` prop highlights a specific message with gold border and auto-scrolls to it
- **Timestamps** — short format for today, full format for older messages

### MessageWithItemLinks

Parses message text to detect `[Item: ItemName]` patterns and renders them as `ItemInline` components with hover tooltips and click-to-navigate behavior.

## Database Schema

| Table | Purpose |
|-------|---------|
| `chat_messages` | Core message storage (timestamp, channel, sender, message, flags) |
| `chat_item_links` | Item references found in messages (raw_text, item_name, item_id) |
| `chat_messages_fts` | Full-text search index on message content |
| `poems` | Recorded poems (author, title, content, recorded_at). Global, not character-scoped; deduped via `UNIQUE(author, title, content)` |

## Tauri Commands

### Import & Tailing
- `scan_chat_logs(path) → ScanResult` — bulk import all chat logs from directory
- `scan_chat_log_file(path) → ScanResult` — import single file
- `tail_chat_log(chat_log_file) → Vec<ChatMessage>` — continuous import of active log

### Query
- `get_chat_messages(ChatFilter) → Vec<ChatMessage>` — filtered message query
- `get_chat_messages_around(message_id, context_count?) → Vec<ChatMessage>` — messages surrounding a target message in the same channel (default 25 before/after)
- `get_chat_channels() → Vec<String>` — list all channels
- `get_chat_channel_stats() → Vec<ChannelStat>` — per-channel message counts
- `get_tell_conversations() → Vec<ChannelStat>` — list conversation partners
- `get_watch_rule_messages(rule_id, limit, offset) → Vec<ChatMessage>` — messages matching a watchword rule
- `get_chat_stats() → ChatStats` — overall statistics
- `get_poems() → Vec<PoemRow>` — all recorded poems (global, newest first); searching/filtering is done on the frontend
- `scan_player_log_for_poems() → usize` — one-time backfill: scans the entire current Player.log for past `Poem by X` recitals and inserts them (deduped). Exposed as the "Scan Player.log" button on the Poems tab, since the live watcher only sees poems recited after it starts tailing.

### Maintenance
- `purge_chat_messages(days) → usize` — delete messages older than N days
- `delete_all_chat_messages() → usize` — wipe all chat data

### ChatFilter

```typescript
interface ChatFilter {
  channel?: string;
  sender?: string;
  searchText?: string;
  startTime?: string;
  endTime?: string;
  hasItemLinks?: boolean;
  itemName?: string;
  tellPartner?: string;
  limit?: number;
  offset?: number;
}
```

## Key Design Decisions

- **FTS indexing** — full-text search via SQLite FTS5 for fast text queries across potentially millions of messages.
- **Item link extraction at parse time** — item references are detected during import and stored in a separate table, enabling "item links only" filtering without re-scanning messages.
- **Watchword rules in settings** — rules persist in `settingsStore` (app settings file) rather than the database, keeping them lightweight and portable.
- **Deduplication** — `INSERT OR IGNORE` prevents duplicate messages when re-importing or tailing overlapping ranges.
- **Offset pagination** — simple offset/limit pagination rather than cursor-based, sufficient for chat browsing patterns.
- **Structured search syntax** — `from:player` and `in:channel` operators parsed on the frontend and mapped to existing backend filter fields. Remaining text goes to FTS5.
- **Context navigation** — clicking a search result loads surrounding messages via `get_chat_messages_around`, allowing users to see the conversation context.
