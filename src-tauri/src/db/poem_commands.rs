use super::DbPool;
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct PoemRow {
    pub id: i64,
    pub author: String,
    pub title: String,
    pub content: String,
    pub recorded_at: String,
}

/// All recorded poems, newest first.
///
/// Poems are global (not scoped to the active character), so no
/// character/server filter is applied — the Poems tab shows every poem ever
/// captured. Searching/filtering is handled on the frontend against this list.
#[tauri::command(rename_all = "camelCase")]
pub fn get_poems(db: State<'_, DbPool>) -> Result<Vec<PoemRow>, String> {
    let conn = db.get().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, author, title, content, recorded_at
             FROM poems
             ORDER BY recorded_at DESC, id DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(PoemRow {
                id: row.get(0)?,
                author: row.get(1)?,
                title: row.get(2)?,
                content: row.get(3)?,
                recorded_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut poems = Vec::new();
    for row in rows {
        poems.push(row.map_err(|e| e.to_string())?);
    }
    Ok(poems)
}
