use super::DbConnection;
/// Database commands for chat message operations
use crate::chat_parser::ChatMessage;
use crate::settings::{ConditionMatch, WatchCondition, WatchRule};
use rusqlite::{params, OptionalExtension, Result};

/// Insert a batch of chat messages into the database.
/// Messages on excluded channels are silently skipped — they must never be stored.
pub fn insert_chat_messages(
    conn: &DbConnection,
    messages: &[ChatMessage],
    log_file: &str,
    excluded_channels: &[String],
) -> Result<usize> {
    let mut inserted = 0;

    for msg in messages {
        // Never store messages from excluded channels
        if let Some(ref channel) = msg.channel {
            if excluded_channels.iter().any(|c| c == channel) {
                continue;
            }
        }

        // Insert the message - use INSERT OR IGNORE to handle duplicates gracefully
        let rows_affected = conn.execute(
            "INSERT OR IGNORE INTO chat_messages (timestamp, channel, sender, message, is_system, log_file, from_player)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                msg.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
                msg.channel,
                msg.sender,
                msg.message,
                msg.is_system,
                log_file,
                msg.from_player
            ],
        )?;

        // Skip if this was a duplicate (no rows inserted)
        if rows_affected == 0 {
            continue;
        }

        // Get the ID of the inserted message
        let message_id = conn.last_insert_rowid();

        // Insert item links if any
        for link in &msg.item_links {
            // Look up the item in the game data by name
            let item_id: Option<i64> = conn
                .query_row(
                    "SELECT id FROM items WHERE name = ?1 COLLATE NOCASE",
                    params![&link.item_name],
                    |row| row.get(0),
                )
                .optional()?;

            conn.execute(
                "INSERT INTO chat_item_links (message_id, raw_text, item_name, item_id)
                 VALUES (?1, ?2, ?3, ?4)",
                params![message_id, &link.raw_text, &link.item_name, item_id],
            )?;
        }

        inserted += 1;
    }

    Ok(inserted)
}

/// Get chat messages with optional filters
#[derive(Debug, Clone)]
pub struct ChatMessageFilter {
    pub channel: Option<String>,
    pub sender: Option<String>,
    pub search_text: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub has_item_links: Option<bool>,
    pub item_name: Option<String>,
    pub tell_partner: Option<String>,
    pub limit: i64,
    pub offset: i64,
    /// "asc" for oldest-first, "desc" (default) for newest-first
    pub sort_order: String,
}

impl Default for ChatMessageFilter {
    fn default() -> Self {
        Self {
            channel: None,
            sender: None,
            search_text: None,
            start_time: None,
            end_time: None,
            has_item_links: None,
            item_name: None,
            tell_partner: None,
            limit: 100,
            offset: 0,
            sort_order: "desc".to_string(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ChatMessageRow {
    pub id: i64,
    pub timestamp: String,
    pub channel: Option<String>,
    pub sender: Option<String>,
    pub message: String,
    pub is_system: bool,
    pub from_player: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub item_links: Vec<ChatItemLinkRow>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ChatItemLinkRow {
    pub raw_text: String,
    pub item_name: String,
    pub item_id: Option<i64>,
}

pub fn get_chat_messages(
    conn: &DbConnection,
    filter: &ChatMessageFilter,
) -> Result<Vec<ChatMessageRow>> {
    eprintln!("[DEBUG] get_chat_messages filter: {:?}", filter);

    let (conditions, params) = build_chat_where(filter);

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    let order_dir = if filter.sort_order == "asc" { "ASC" } else { "DESC" };
    let query = format!(
        "SELECT cm.id, cm.timestamp, cm.channel, cm.sender, cm.message, cm.is_system, cm.from_player \
         FROM chat_messages cm {} ORDER BY cm.timestamp {}, cm.id {} LIMIT {} OFFSET {}",
        where_clause, order_dir, order_dir, filter.limit, filter.offset
    );

    eprintln!("[DEBUG] Chat query: {}", query);

    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query_map(params_refs.as_slice(), |row| {
        Ok(ChatMessageRow {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            channel: row.get(2)?,
            sender: row.get(3)?,
            message: row.get(4)?,
            is_system: row.get(5)?,
            from_player: row.get(6)?,
            item_links: Vec::new(),
        })
    })?;

    let mut messages = Vec::new();
    for row in rows {
        let mut msg = row?;
        msg.item_links = get_item_links_for_message(conn, msg.id)?;
        messages.push(msg);
    }

    Ok(messages)
}

/// Count chat messages matching the same filter semantics as `get_chat_messages`
/// (shared WHERE builder, including the FTS MATCH path).
pub fn count_chat_messages(
    conn: &DbConnection,
    filter: &ChatMessageFilter,
) -> Result<i64> {
    let (conditions, params) = build_chat_where(filter);

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    let query = format!(
        "SELECT COUNT(*) FROM chat_messages cm{}",
        where_clause
    );

    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&query)?;
    let count: i64 = stmt.query_row(params_refs.as_slice(), |row| row.get(0))?;
    Ok(count)
}

/// Shared WHERE builder for chat message queries: conditions + bound params,
/// with the FTS MATCH condition already slotted in. Used by the row query and
/// the count query so both always agree on filter semantics.
fn build_chat_where(
    filter: &ChatMessageFilter,
) -> (Vec<String>, Vec<Box<dyn rusqlite::ToSql>>) {
    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut param_idx = 1;

    // Tell partner filter: automatically sets channel to Tell and sender to partner
    if let Some(partner) = &filter.tell_partner {
        conditions.push(format!("cm.channel = ?{}", param_idx));
        params.push(Box::new("Tell".to_string()));
        param_idx += 1;

        conditions.push(format!("cm.sender = ?{}", param_idx));
        params.push(Box::new(partner.clone()));
        param_idx += 1;
    } else {
        if let Some(channel) = &filter.channel {
            conditions.push(format!("cm.channel = ?{} COLLATE NOCASE", param_idx));
            params.push(Box::new(channel.clone()));
            param_idx += 1;
        }

        if let Some(sender) = &filter.sender {
            conditions.push(format!("cm.sender = ?{} COLLATE NOCASE", param_idx));
            params.push(Box::new(sender.clone()));
            param_idx += 1;
        }
    }

    if let Some(start_time) = &filter.start_time {
        conditions.push(format!("cm.timestamp >= ?{}", param_idx));
        params.push(Box::new(start_time.clone()));
        param_idx += 1;
    }

    if let Some(end_time) = &filter.end_time {
        conditions.push(format!("cm.timestamp <= ?{}", param_idx));
        params.push(Box::new(end_time.clone()));
        param_idx += 1;
    }

    if let Some(has_links) = filter.has_item_links {
        if has_links {
            conditions.push(
                "EXISTS (SELECT 1 FROM chat_item_links cil WHERE cil.message_id = cm.id)"
                    .to_string(),
            );
        } else {
            conditions.push(
                "NOT EXISTS (SELECT 1 FROM chat_item_links cil WHERE cil.message_id = cm.id)"
                    .to_string(),
            );
        }
    }

    if let Some(item_name) = &filter.item_name {
        conditions.push(format!(
            "EXISTS (SELECT 1 FROM chat_item_links cil WHERE cil.message_id = cm.id AND cil.item_name LIKE ?{})",
            param_idx
        ));
        params.push(Box::new(format!("%{}%", item_name)));
        param_idx += 1;
    }

    // Text search: FTS5 index when the query parses as plain words/phrases,
    // LIKE fallback when it contains FTS syntax the user is experimenting with.
    let mut fts_sql: Option<(String, String)> = None; // (sql, match_param)
    if let Some(search_text) = &filter.search_text {
        let text = search_text.trim();
        if !text.is_empty() {
            match build_fts_match_expr(text) {
                Some(match_expr) => {
                    fts_sql = Some((
                        "cm.id IN (SELECT rowid FROM chat_messages_fts WHERE chat_messages_fts MATCH ?{IDX})"
                            .to_string(),
                        match_expr,
                    ));
                }
                None => {
                    // FTS syntax error / unsupported operators: fall back to LIKE per word
                    for word in text.split_whitespace() {
                        let clean: String = word.chars().filter(|c| *c != '%' && *c != '_').collect();
                        if !clean.is_empty() {
                            conditions.push(format!("cm.message LIKE ?{}", param_idx));
                            params.push(Box::new(format!("%{}%", clean)));
                            param_idx += 1;
                        }
                    }
                }
            }
        }
    }

    // Slot the FTS MATCH condition into the AND chain at the next free parameter slot
    if let Some((sql, match_param)) = &fts_sql {
        let sql = sql.replace("{IDX}", &param_idx.to_string());
        params.push(Box::new(match_param.clone()));
        conditions.push(sql);
    }

    (conditions, params)
}


/// Get item links for a specific message
fn get_item_links_for_message(
    conn: &DbConnection,
    message_id: i64,
) -> Result<Vec<ChatItemLinkRow>> {
    let mut stmt = conn.prepare(
        "SELECT raw_text, item_name, item_id
         FROM chat_item_links
         WHERE message_id = ?1
         ORDER BY id",
    )?;

    let rows = stmt.query_map([message_id], |row| {
        Ok(ChatItemLinkRow {
            raw_text: row.get(0)?,
            item_name: row.get(1)?,
            item_id: row.get(2)?,
        })
    })?;

    let mut links = Vec::new();
    for row in rows {
        links.push(row?);
    }

    Ok(links)
}

/// Get messages around a specific message for context viewing.
/// Returns `context_count` messages before and after the target message
/// in the same channel, ordered chronologically.
pub fn get_messages_around(
    conn: &DbConnection,
    message_id: i64,
    context_count: i64,
) -> Result<Vec<ChatMessageRow>> {
    let query = "
        WITH target AS (
            SELECT timestamp, channel FROM chat_messages WHERE id = ?1
        )
        SELECT cm.id, cm.timestamp, cm.channel, cm.sender, cm.message, cm.is_system, cm.from_player
        FROM chat_messages cm, target t
        WHERE cm.channel = t.channel
          AND (
            (cm.timestamp < t.timestamp AND cm.id IN (
                SELECT id FROM chat_messages
                WHERE channel = t.channel AND timestamp <= t.timestamp AND id != ?1
                ORDER BY timestamp DESC LIMIT ?2
            ))
            OR cm.id = ?1
            OR (cm.timestamp > t.timestamp AND cm.id IN (
                SELECT id FROM chat_messages
                WHERE channel = t.channel AND timestamp >= t.timestamp AND id != ?1
                ORDER BY timestamp ASC LIMIT ?2
            ))
          )
        ORDER BY cm.timestamp ASC
    ";

    let mut stmt = conn.prepare(query)?;
    let rows = stmt.query_map(rusqlite::params![message_id, context_count], |row| {
        Ok(ChatMessageRow {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            channel: row.get(2)?,
            sender: row.get(3)?,
            message: row.get(4)?,
            is_system: row.get(5)?,
            from_player: row.get(6)?,
            item_links: Vec::new(),
        })
    })?;

    let mut messages = Vec::new();
    for row in rows {
        let mut msg = row?;
        msg.item_links = get_item_links_for_message(conn, msg.id)?;
        messages.push(msg);
    }

    Ok(messages)
}

/// Build an FTS5 MATCH expression from a user search string.
///
/// Plain words become quoted tokens joined implicitly with AND. Phrases in
/// double quotes become quoted FTS phrases. A trailing `*` becomes a prefix
/// query (`"gorg"*`). Tokens containing FTS syntax (parens, column filters,
/// `NEAR`, stray operators) are rejected with `None` so the caller can fall
/// back to LIKE substring matching instead of surfacing an FTS error.
pub fn build_fts_match_expr(search_text: &str) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut chars = search_text.chars().peekable();

    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if c == '"' {
            // Quoted phrase: consume up to the closing quote
            chars.next();
            let mut phrase = String::new();
            let mut closed = false;
            for ch in chars.by_ref() {
                if ch == '"' {
                    closed = true;
                    break;
                }
                phrase.push(ch);
            }
            let phrase = phrase.trim();
            if !phrase.is_empty() {
                parts.push(format!("\"{}\"", phrase.replace('"', "")));
            }
            if !closed {
                // Unterminated quote: treat the whole query as FTS-malformed
                return None;
            }
        } else {
            // Bare word: consume until whitespace. Only a single trailing `*`
            // forms a prefix; a star anywhere else (leading, internal,
            // repeated) is unsupported syntax — bail out so the query takes
            // the LIKE fallback instead of silently matching something else
            // (e.g. `go*rg` must not become `"gorg"*`).
            let mut word = String::new();
            let mut saw_star = false;
            let mut star_ended_word = false;
            for ch in chars.by_ref() {
                if ch.is_whitespace() {
                    break;
                }
                if ch == '"' {
                    // Quote in the middle of a word: FTS syntax, bail out
                    return None;
                }
                if ch == '*' {
                    if saw_star {
                        // Repeated star (`gorg**`): unsupported
                        return None;
                    }
                    saw_star = true;
                    star_ended_word = true;
                    continue;
                }
                if saw_star {
                    // Star in the middle (`go*rg`): unsupported
                    return None;
                }
                star_ended_word = false;
                if "()^:,+-".contains(ch) {
                    // FTS operators / column filters: not plain words
                    return None;
                }
                word.push(ch);
            }
            let word = word.trim();
            if word.is_empty() {
                if saw_star {
                    return None; // bare `*` is an FTS syntax error
                }
                continue;
            }
            if saw_star && star_ended_word {
                parts.push(format!("\"{}\"*", word));
            } else {
                parts.push(format!("\"{}\"", word));
            }
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

/// Days (UTC, from stored timestamps) that contain chat messages, newest first.
pub fn get_chat_days(conn: &DbConnection) -> Result<Vec<ChatDayRow>> {
    let mut stmt = conn.prepare(
        "SELECT substr(timestamp, 1, 10) AS day, COUNT(*) AS count
         FROM chat_messages
         GROUP BY day
         ORDER BY day DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(ChatDayRow {
            day: row.get(0)?,
            count: row.get(1)?,
        })
    })?;

    let mut days = Vec::new();
    for row in rows {
        days.push(row?);
    }

    Ok(days)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ChatDayRow {
    /// UTC day as `YYYY-MM-DD`
    pub day: String,
    pub count: i64,
}

/// Get messages starting at a time anchor: a DAY anchor (`YYYY-MM-DD`)
/// returns that day's rows in the requested sort order; a full-timestamp
/// anchor returns up to `context_count` rows at-or-before (desc) or
/// at-or-after (asc) the anchor, in the requested sort order. Caller
/// filters (channel, sender, search) always apply. No offset math — uses
/// the timestamp index for O(log n) boundary seeks.
pub fn get_messages_around_time(
    conn: &DbConnection,
    anchor_time: &str,
    filter: &ChatMessageFilter,
    context_count: i64,
) -> Result<Vec<ChatMessageRow>> {
    // Accept a bare `YYYY-MM-DD` day (day-jump case) or a full timestamp.
    let day = if anchor_time.len() == 10 {
        anchor_time.to_string()
    } else {
        anchor_time.get(..10).unwrap_or(anchor_time).to_string()
    };

    // Day-jump semantics: the window is that DAY's rows in the requested
    // sort order — never leaks adjacent days, and the ordering matches the
    // sort toggle so continuation pagination composes cleanly. A full
    // timestamp anchor bounds the window at that instant instead (desc:
    // at-or-before the anchor, asc: at-or-after). Bounds and caller filters
    // build in ONE filter so parameter indexes stay contiguous.
    let day_only = anchor_time.len() == 10;
    let desc = filter.sort_order != "asc";
    let mut combined = filter.clone();
    combined.start_time = Some(format!("{} 00:00:00", day));
    combined.end_time = Some(format!("{} 23:59:59", day));
    combined.limit = context_count;
    combined.offset = 0;
    let (mut conditions, mut params) = build_chat_where(&combined);

    if !day_only {
        let anchor_cond = if desc {
            format!("cm.timestamp <= ?{}", params.len() + 1)
        } else {
            format!("cm.timestamp >= ?{}", params.len() + 1)
        };
        conditions.push(anchor_cond);
        params.push(Box::new(anchor_time.to_string()));
    }

    let order = if desc {
        "ORDER BY cm.timestamp DESC, cm.id DESC"
    } else {
        "ORDER BY cm.timestamp ASC, cm.id ASC"
    };

    let sql = format!(
        "SELECT cm.id, cm.timestamp, cm.channel, cm.sender, cm.message, cm.is_system, cm.from_player \
         FROM chat_messages cm WHERE {} {} LIMIT {}",
        conditions.join(" AND "),
        order,
        context_count,
    );

    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_refs.as_slice(), map_chat_row)?;
    let mut messages = Vec::new();
    for row in rows {
        let mut msg = row?;
        msg.item_links = get_item_links_for_message(conn, msg.id)?;
        messages.push(msg);
    }
    // The window's order matches the requested sort, so the label and
    // continuation offsets agree with get_chat_messages.
    Ok(messages)
}

fn map_chat_row(row: &rusqlite::Row<'_>) -> Result<ChatMessageRow> {
    Ok(ChatMessageRow {
        id: row.get(0)?,
        timestamp: row.get(1)?,
        channel: row.get(2)?,
        sender: row.get(3)?,
        message: row.get(4)?,
        is_system: row.get(5)?,
        from_player: row.get(6)?,
        item_links: Vec::new(),
    })
}

/// Force a full rebuild of the chat FTS index (call after bulk backfills so
/// the index is guaranteed consistent even if a historical trigger was missed).
pub fn rebuild_chat_fts(conn: &DbConnection) -> Result<()> {
    conn.execute("INSERT INTO chat_messages_fts(chat_messages_fts) VALUES('rebuild')", [])?;
    Ok(())
}

/// Get unique channels
pub fn get_channels(conn: &DbConnection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT channel FROM chat_messages
         WHERE channel IS NOT NULL
         ORDER BY channel",
    )?;

    let rows = stmt.query_map([], |row| row.get(0))?;

    let mut channels = Vec::new();
    for row in rows {
        channels.push(row?);
    }

    Ok(channels)
}

/// Get message count by channel
pub fn get_channel_stats(conn: &DbConnection) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT channel, COUNT(*) as count
         FROM chat_messages
         WHERE channel IS NOT NULL
         GROUP BY channel
         ORDER BY count DESC",
    )?;

    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;

    let mut stats = Vec::new();
    for row in rows {
        stats.push(row?);
    }

    Ok(stats)
}

/// Get overall chat statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChatStats {
    pub total_messages: i64,
    pub channel_count: i64,
    pub oldest_message: String,
    pub newest_message: String,
    pub database_size_bytes: i64,
    pub messages_size_bytes: i64,
    pub item_links_count: i64,
}

pub fn get_chat_stats(conn: &DbConnection) -> Result<ChatStats> {
    let total_messages: i64 = conn
        .query_row("SELECT COUNT(*) FROM chat_messages", [], |row| row.get(0))
        .unwrap_or(0);

    let channel_count: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT channel) FROM chat_messages WHERE channel IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let oldest_message: String = conn
        .query_row(
            "SELECT timestamp FROM chat_messages ORDER BY timestamp ASC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "N/A".to_string());

    let newest_message: String = conn
        .query_row(
            "SELECT timestamp FROM chat_messages ORDER BY timestamp DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "N/A".to_string());

    let item_links_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM chat_item_links", [], |row| row.get(0))
        .unwrap_or(0);

    // Sum the size of chat-related tables and indexes only
    let messages_size_bytes: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(pgsize), 0) FROM dbstat WHERE name LIKE 'chat_%'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let database_size_bytes: i64 = conn
        .query_row(
            "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(ChatStats {
        total_messages,
        channel_count,
        oldest_message,
        newest_message,
        database_size_bytes,
        messages_size_bytes,
        item_links_count,
    })
}

/// Get list of unique conversation partners from Tell messages
pub fn get_tell_conversations(conn: &DbConnection) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT sender, COUNT(*) as count
         FROM chat_messages
         WHERE channel = 'Tell' AND sender IS NOT NULL
         GROUP BY sender
         ORDER BY MAX(timestamp) DESC",
    )?;

    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;

    let mut conversations = Vec::new();
    for row in rows {
        conversations.push(row?);
    }

    Ok(conversations)
}

/// Query chat messages that match a watch rule's conditions.
///
/// Builds a SQL query from the rule's conditions:
/// - ContainsText: matches message body OR item link names (case-insensitive)
/// - ContainsItemLink: matches item link names only
/// - FromSender: exact sender match (case-insensitive)
/// - Channel filter: restricts to specified channels
pub fn get_watch_rule_messages(
    conn: &DbConnection,
    rule: &WatchRule,
    limit: i64,
    offset: i64,
    excluded_channels: &[String],
) -> Result<Vec<ChatMessageRow>> {
    let mut conditions: Vec<String> = Vec::new();
    let mut param_values: Vec<String> = Vec::new();
    let mut param_idx = 1;

    // Exclude messages from excluded channels (they shouldn't be in the DB,
    // but may exist from before the channel was excluded)
    if !excluded_channels.is_empty() {
        let placeholders: Vec<String> = excluded_channels
            .iter()
            .map(|_| {
                let p = format!("?{}", param_idx);
                param_idx += 1;
                p
            })
            .collect();
        conditions.push(format!(
            "(cm.channel IS NULL OR cm.channel NOT IN ({}))",
            placeholders.join(", ")
        ));
        for ch in excluded_channels {
            param_values.push(ch.clone());
        }
    }

    // Channel filter
    if let Some(ref channels) = rule.channels {
        if !channels.is_empty() {
            let placeholders: Vec<String> = channels
                .iter()
                .map(|_| {
                    let p = format!("?{}", param_idx);
                    param_idx += 1;
                    p
                })
                .collect();
            conditions.push(format!("cm.channel IN ({})", placeholders.join(", ")));
            for ch in channels {
                param_values.push(ch.clone());
            }
        }
    }

    // Build watch conditions
    let mut watch_conditions: Vec<String> = Vec::new();
    for condition in &rule.conditions {
        match condition {
            WatchCondition::ContainsText(text) => {
                let like_param = format!("%{}%", text);
                // Match in message body OR in item link names
                watch_conditions.push(format!(
                    "(cm.message LIKE ?{} COLLATE NOCASE OR EXISTS (\
                        SELECT 1 FROM chat_item_links cil \
                        WHERE cil.message_id = cm.id AND cil.item_name LIKE ?{} COLLATE NOCASE\
                    ))",
                    param_idx,
                    param_idx + 1
                ));
                param_values.push(like_param.clone());
                param_values.push(like_param);
                param_idx += 2;
            }
            WatchCondition::ContainsItemLink(item_name) => {
                let like_param = format!("%{}%", item_name);
                watch_conditions.push(format!(
                    "EXISTS (\
                        SELECT 1 FROM chat_item_links cil \
                        WHERE cil.message_id = cm.id AND cil.item_name LIKE ?{} COLLATE NOCASE\
                    )",
                    param_idx
                ));
                param_values.push(like_param);
                param_idx += 1;
            }
            WatchCondition::FromSender(sender) => {
                watch_conditions.push(format!("cm.sender = ?{} COLLATE NOCASE", param_idx));
                param_values.push(sender.clone());
                param_idx += 1;
            }
        }
    }

    // Join watch conditions with AND or OR based on match_mode
    if !watch_conditions.is_empty() {
        let joiner = match rule.match_mode {
            ConditionMatch::All => " AND ",
            ConditionMatch::Any => " OR ",
        };
        conditions.push(format!("({})", watch_conditions.join(joiner)));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    let query = format!(
        "SELECT cm.id, cm.timestamp, cm.channel, cm.sender, cm.message, cm.is_system, cm.from_player \
         FROM chat_messages cm{} ORDER BY cm.timestamp DESC LIMIT {} OFFSET {}",
        where_clause, limit, offset
    );

    let params_refs: Vec<&dyn rusqlite::ToSql> = param_values
        .iter()
        .map(|p| p as &dyn rusqlite::ToSql)
        .collect();

    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query_map(params_refs.as_slice(), |row| {
        Ok(ChatMessageRow {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            channel: row.get(2)?,
            sender: row.get(3)?,
            message: row.get(4)?,
            is_system: row.get(5)?,
            from_player: row.get(6)?,
            item_links: Vec::new(),
        })
    })?;

    let mut messages = Vec::new();
    for row in rows {
        let mut msg = row?;
        msg.item_links = get_item_links_for_message(conn, msg.id)?;
        messages.push(msg);
    }

    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::run_migrations;
    use r2d2::Pool;
    use r2d2_sqlite::SqliteConnectionManager;

    fn setup() -> DbConnection {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder().build(manager).unwrap();
        let conn = pool.get().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&conn, None).unwrap();
        conn
    }

    fn msg_with(conn: &DbConnection, timestamp: &str, channel: &str, sender: &str, message: &str) {
        conn.execute(
            "INSERT INTO chat_messages (timestamp, channel, sender, message, is_system, log_file, from_player)
             VALUES (?1, ?2, ?3, ?4, 0, 'Chat-test.log', 0)",
            rusqlite::params![timestamp, channel, sender, message],
        )
        .unwrap();
    }

    fn filter_with(search_text: Option<&str>) -> ChatMessageFilter {
        ChatMessageFilter {
            search_text: search_text.map(|s| s.to_string()),
            ..Default::default()
        }
    }

    fn filter_sort(sort_order: &str) -> ChatMessageFilter {
        ChatMessageFilter {
            sort_order: sort_order.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_build_fts_match_expr_plain_words() {
        let expr = build_fts_match_expr("hello world").unwrap();
        assert_eq!(expr, r#""hello" "world""#);
    }

    #[test]
    fn test_build_fts_match_expr_phrase() {
        let expr = build_fts_match_expr("say \"exact phrase\" now").unwrap();
        assert_eq!(expr, r#""say" "exact phrase" "now""#);
    }

    #[test]
    fn test_build_fts_match_expr_prefix() {
        let expr = build_fts_match_expr("gorg*").unwrap();
        assert_eq!(expr, r#""gorg"*"#);
        // Multi-word with a trailing prefix on the last word
        let expr = build_fts_match_expr("gorgon ore*").unwrap();
        assert_eq!(expr, r#""gorgon" "ore"*"#);
    }

    #[test]
    fn test_build_fts_match_expr_rejects_misplaced_wildcards() {
        // Only a single TRAILING `*` is a prefix. Leading/internal/repeated
        // stars are unsupported syntax — None ⇒ LIKE fallback, so `go*rg`
        // never silently becomes `"gorg"*`.
        assert!(build_fts_match_expr("go*rg").is_none());
        assert!(build_fts_match_expr("*gorg").is_none());
        assert!(build_fts_match_expr("gorg**").is_none());
        assert!(build_fts_match_expr("g*o*r*g").is_none());
        // A valid prefix next to a bad token rejects the whole query too.
        assert!(build_fts_match_expr("gorg* go*rg").is_none());
    }

    #[test]
    fn test_build_fts_match_expr_rejects_fts_syntax() {
        assert!(build_fts_match_expr("NEAR(a b, 5)").is_none());
        assert!(build_fts_match_expr("message:hello").is_none());
        assert!(build_fts_match_expr("(a OR b)").is_none());
        assert!(build_fts_match_expr("a \"unclosed").is_none());
        assert!(build_fts_match_expr("*").is_none());
    }

    #[test]
    fn test_build_fts_match_expr_edge_cases() {
        assert!(build_fts_match_expr("").is_none());
        assert!(build_fts_match_expr("   ").is_none());
        // Unclosed quote: FTS-malformed → None → LIKE fallback
        assert!(build_fts_match_expr("hello \"unclosed").is_none());
    }

    #[test]
    fn test_fts_search_finds_words_case_insensitively() {
        let conn = setup();
        msg_with(&conn, "2026-07-01 10:00:00", "General", "Alice", "Hello Gorgon fans");
        msg_with(&conn, "2026-07-01 10:01:00", "General", "Bob", "nothing to see here");
        msg_with(&conn, "2026-07-02 10:00:00", "Trade", "Cara", "GORGON rises again");

        let messages = get_chat_messages(&conn, &filter_with(Some("gorgon"))).unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_fts_search_multiple_words_are_anded() {
        let conn = setup();
        msg_with(&conn, "2026-07-01 10:00:00", "General", "Alice", "hello world");
        msg_with(&conn, "2026-07-01 10:01:00", "General", "Bob", "hello there");

        let messages = get_chat_messages(&conn, &filter_with(Some("hello world"))).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message, "hello world");
    }

    #[test]
    fn test_fts_search_phrase_requires_adjacency() {
        let conn = setup();
        msg_with(&conn, "2026-07-01 10:00:00", "General", "Alice", "world hello");
        msg_with(&conn, "2026-07-01 10:01:00", "General", "Bob", "hello world");

        let messages = get_chat_messages(&conn, &filter_with(Some("\"hello world\""))).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender.as_deref().unwrap(), "Bob");
    }

    #[test]
    fn test_fts_prefix_query() {
        let conn = setup();
        msg_with(&conn, "2026-07-01 10:00:00", "General", "Alice", "gorgonite says hi");
        msg_with(&conn, "2026-07-01 10:01:00", "General", "Bob", "no match here");

        let messages = get_chat_messages(&conn, &filter_with(Some("gorg*"))).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender.as_deref().unwrap(), "Alice");
    }

    #[test]
    fn test_like_fallback_on_fts_syntax() {
        let conn = setup();
        msg_with(&conn, "2026-07-01 10:00:00", "General", "Alice", "message:hello friends");
        msg_with(&conn, "2026-07-01 10:01:00", "General", "Bob", "hello again");

        // `message:hello` would be a column filter in FTS; falls back to LIKE substring
        let messages = get_chat_messages(&conn, &filter_with(Some("message:hello"))).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender.as_deref().unwrap(), "Alice");
    }

    #[test]
    fn test_fts_combined_with_channel_filter() {
        let conn = setup();
        msg_with(&conn, "2026-07-01 10:00:00", "General", "Alice", "gorgon for sale");
        msg_with(&conn, "2026-07-01 10:01:00", "Trade", "Bob", "gorgon for sale");

        let filter = ChatMessageFilter {
            channel: Some("Trade".to_string()),
            ..filter_with(Some("gorgon"))
        };
        let messages = get_chat_messages(&conn, &filter).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].channel.as_deref().unwrap(), "Trade");
    }

    #[test]
    fn test_fts_combined_with_time_range() {
        let conn = setup();
        msg_with(&conn, "2026-07-01 10:00:00", "General", "Alice", "gorgon early");
        msg_with(&conn, "2026-07-05 10:00:00", "General", "Bob", "gorgon late");

        let filter = ChatMessageFilter {
            start_time: Some("2026-07-03".to_string()),
            ..filter_with(Some("gorgon"))
        };
        let messages = get_chat_messages(&conn, &filter).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender.as_deref().unwrap(), "Bob");
    }

    #[test]
    fn test_fts_searches_sender_column_via_query() {
        // FTS indexes message+sender; a plain word may match the sender only
        let conn = setup();
        msg_with(&conn, "2026-07-01 10:00:00", "General", "Zaxxas", "selling ores");

        let messages = get_chat_messages(&conn, &filter_with(Some("zaxxas"))).unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_fts_syntax_like_fallback_matches_nothing_safely() {
        // Hyphen-prefixed token is FTS column-op shape → LIKE fallback; no SQL error
        let conn = setup();
        msg_with(&conn, "2026-07-01 10:00:00", "General", "Alice", "plain message");
        msg_with(&conn, "2026-07-01 10:01:00", "General", "Bob", "plain message");

        let filter = ChatMessageFilter {
            search_text: Some("-message".to_string()),
            ..Default::default()
        };
        let messages = get_chat_messages(&conn, &filter).unwrap();
        assert_eq!(messages.len(), 0);
    }

    #[test]
    fn test_fts_parens_like_fallback_matches_raw_text() {
        let conn = setup();
        msg_with(&conn, "2026-07-01 10:00:00", "General", "Alice", "counting (one) two");
        msg_with(&conn, "2026-07-01 10:01:00", "General", "Bob", "counting three four");

        // Parens are FTS syntax → LIKE fallback over the raw text
        let messages = get_chat_messages(&conn, &filter_with(Some("(one)"))).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender.as_deref().unwrap(), "Alice");
    }

    #[test]
    fn test_fts_search_sort_order_respected() {
        let conn = setup();
        msg_with(&conn, "2026-07-01 10:00:00", "General", "Alice", "gorgon one");
        msg_with(&conn, "2026-07-01 10:01:00", "General", "Bob", "gorgon two");
        msg_with(&conn, "2026-07-01 10:02:00", "General", "Cara", "gorgon three");

        let filter = ChatMessageFilter {
            sort_order: "asc".to_string(),
            ..filter_with(Some("gorgon"))
        };
        let messages = get_chat_messages(&conn, &filter).unwrap();
        assert_eq!(messages[0].message, "gorgon one");
        assert_eq!(messages[2].message, "gorgon three");
    }

    #[test]
    fn test_fts_search_respects_limit_offset() {
        let conn = setup();
        for i in 0..5 {
            msg_with(&conn, &format!("2026-07-01 10:0{}:00", i), "General", "A", "gorgon spam");
        }
        let filter = ChatMessageFilter {
            limit: 2,
            offset: 1,
            ..filter_with(Some("gorgon"))
        };
        let messages = get_chat_messages(&conn, &filter).unwrap();
        assert_eq!(messages.len(), 2);
        // desc order: newest first; offset skips the newest
        assert_eq!(messages[0].timestamp, "2026-07-01 10:03:00");
    }

    #[test]
    fn test_get_chat_days() {
        let conn = setup();
        msg_with(&conn, "2026-07-01 10:00:00", "General", "A", "one");
        msg_with(&conn, "2026-07-01 15:30:00", "General", "B", "two");
        msg_with(&conn, "2026-07-03 09:00:00", "General", "C", "three");

        let days = get_chat_days(&conn).unwrap();
        assert_eq!(days.len(), 2);
        assert_eq!(days[0].day, "2026-07-03");
        assert_eq!(days[0].count, 1);
        assert_eq!(days[1].day, "2026-07-01");
        assert_eq!(days[1].count, 2);
    }

    #[test]
    fn test_get_messages_around_time_day_anchor() {
        let conn = setup();
        // Day before, day of, day after
        msg_with(&conn, "2026-06-30 22:00:00", "General", "A", "before day");
        msg_with(&conn, "2026-07-01 09:00:00", "General", "B", "morning");
        msg_with(&conn, "2026-07-01 18:00:00", "General", "C", "evening");
        msg_with(&conn, "2026-07-02 09:00:00", "General", "D", "next day");

        // Day-window semantics: the day's rows in the requested sort order,
        // never leaking adjacent days (A, D excluded). Desc with context 2:
        // newest two rows of the day.
        let messages =
            get_messages_around_time(&conn, "2026-07-01", &filter_sort("desc"), 2).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].sender.as_deref().unwrap(), "C");
        assert_eq!(messages[1].sender.as_deref().unwrap(), "B");
        // Context covering the whole day
        let messages =
            get_messages_around_time(&conn, "2026-07-01", &filter_sort("desc"), 5).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].sender.as_deref().unwrap(), "C");
        assert_eq!(messages[1].sender.as_deref().unwrap(), "B");
        // Oldest-first ordering honors sort_order
        let messages =
            get_messages_around_time(&conn, "2026-07-01", &filter_sort("asc"), 5).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].sender.as_deref().unwrap(), "B");
        assert_eq!(messages[1].sender.as_deref().unwrap(), "C");
    }

    #[test]
    fn test_get_messages_around_time_channel_filter() {
        let conn = setup();
        msg_with(&conn, "2026-06-30 22:00:00", "General", "A", "gen before");
        msg_with(&conn, "2026-07-01 09:00:00", "Trade", "B", "trade one");
        msg_with(&conn, "2026-07-01 12:00:00", "General", "C", "gen mid");
        msg_with(&conn, "2026-07-01 15:00:00", "Trade", "D", "trade two");
        msg_with(&conn, "2026-07-01 18:00:00", "Trade", "E", "trade three");

        let filter = ChatMessageFilter {
            channel: Some("Trade".to_string()),
            ..Default::default()
        };
        let messages = get_messages_around_time(&conn, "2026-07-01", &filter, 25).unwrap();
        // Trade only, day-bounded, newest-first: E, D, B
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].sender.as_deref().unwrap(), "E");
        assert_eq!(messages[1].sender.as_deref().unwrap(), "D");
        assert_eq!(messages[2].sender.as_deref().unwrap(), "B");
    }

    #[test]
    fn test_get_messages_around_time_search_filter() {
        let conn = setup();
        msg_with(&conn, "2026-07-01 09:00:00", "General", "A", "gorgon sighting");
        msg_with(&conn, "2026-07-01 11:00:00", "General", "B", "unrelated chat");
        msg_with(&conn, "2026-07-01 18:00:00", "General", "C", "another gorgon");

        let filter = ChatMessageFilter {
            search_text: Some("gorgon".to_string()),
            ..Default::default()
        };
        let messages = get_messages_around_time(&conn, "2026-07-01", &filter, 25).unwrap();
        // Search filter applies to the day window: A and C only, newest-first
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].sender.as_deref().unwrap(), "C");
        assert_eq!(messages[1].sender.as_deref().unwrap(), "A");
    }

    #[test]
    fn test_get_messages_around_time_empty_day_is_empty() {
        let conn = setup();
        msg_with(&conn, "2026-07-01 10:00:00", "General", "A", "only day");
        msg_with(&conn, "2026-07-05 10:00:00", "General", "B", "later day");

        // Anchor on a day with no messages: empty — a day jump never
        // fabricates rows from other days.
        let messages =
            get_messages_around_time(&conn, "2026-06-15", &ChatMessageFilter::default(), 25)
                .unwrap();
        assert_eq!(messages.len(), 0);
    }

    #[test]
    fn test_get_messages_around_time_before_first_row() {
        let conn = setup();
        msg_with(&conn, "2026-07-01 10:00:00", "General", "A", "first");
        msg_with(&conn, "2026-07-01 11:00:00", "General", "B", "second");

        // Oldest-first window from the day start
        let messages =
            get_messages_around_time(&conn, "2026-07-01", &filter_sort("asc"), 25).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].sender.as_deref().unwrap(), "A");
        assert_eq!(messages[1].sender.as_deref().unwrap(), "B");
    }

    #[test]
    fn test_rebuild_chat_fts_and_search_after_bulk_insert() {
        let conn = setup();
        msg_with(&conn, "2026-07-01 10:00:00", "General", "A", "rebuild me");
        // Simulate a missed trigger: manually gut the index, then rebuild
        conn.execute("DELETE FROM chat_messages_fts", []).unwrap();
        rebuild_chat_fts(&conn).unwrap();

        let messages = get_chat_messages(&conn, &filter_with(Some("rebuild"))).unwrap();
        assert_eq!(messages.len(), 1);
    }
}
