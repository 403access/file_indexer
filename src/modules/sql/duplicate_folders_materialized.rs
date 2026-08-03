use rusqlite::Connection;
use std::collections::HashMap;

use crate::modules::logging;

#[derive(Debug, Clone)]
pub struct FolderGroupRow {
    pub group_id: String,
    pub folder_path: String,
    pub folder_name: String,
    pub shared_count: usize,
    pub file_count: usize,
}

pub fn create_duplicate_folder_groups_table(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS duplicate_folder_groups (
            id INTEGER PRIMARY KEY,
            group_id TEXT NOT NULL,
            folder_path TEXT NOT NULL,
            folder_name TEXT,
            shared_count INTEGER NOT NULL DEFAULT 0,
            file_count INTEGER NOT NULL DEFAULT 0,
            updated_at REAL NOT NULL,
            UNIQUE(group_id, folder_path)
        );",
        [],
    )?;
    Ok(())
}

pub fn refresh_duplicate_folder_groups(conn: &Connection) {
    logging::info("Refreshing duplicate folder groups...");

    let _ = conn.execute("DELETE FROM duplicate_folder_groups", []);

    let mut folder_hashes: HashMap<String, Vec<String>> = HashMap::new();
    let mut folder_names: HashMap<String, String> = HashMap::new();

    let mut stmt = conn
        .prepare(
            "SELECT f.path, fn.name, f.hash
             FROM files f
             JOIN file_names fn ON f.file_name_id = fn.id
             WHERE f.hash IN (SELECT hash FROM duplicate_hashes)
               AND f.hash IS NOT NULL AND f.hash != ''
               AND f.parent_path IS NOT NULL",
        )
        .ok();

    if let Some(ref mut s) = stmt {
        let rows = s.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        }).ok();

        if let Some(rows) = rows {
            for row in rows {
                if let Ok((path, _name, hash)) = row {
                    let normalized = path.replace("//", "/");
                    if let Some(parent) = normalized.rsplit_once('/') {
                        let folder = parent.0.to_string();
                        folder_hashes.entry(folder.clone()).or_default().push(hash);
                        if let Some(display) = folder.rsplit_once('/') {
                            folder_names.entry(folder.clone()).or_insert_with(|| display.1.to_string());
                        }
                    }
                }
            }
        }
    }

    if folder_hashes.is_empty() {
        logging::info("No duplicate folder groups found");
        return;
    }

    let folder_list: Vec<(String, Vec<String>)> = folder_hashes.into_iter().collect();
    let mut used = vec![false; folder_list.len()];
    let mut groups: Vec<Vec<(String, Vec<String>)>> = Vec::new();

    for i in 0..folder_list.len() {
        if used[i] {
            continue;
        }
        let mut group = vec![folder_list[i].clone()];
        used[i] = true;

        for j in (i + 1)..folder_list.len() {
            if used[j] {
                continue;
            }
            if group.iter().any(|g| {
                g.1.iter()
                    .any(|h| folder_list[j].1.contains(h))
            }) {
                group.push(folder_list[j].clone());
                used[j] = true;
            }
        }

        if group.len() >= 2 {
            groups.push(group);
        }
    }

    let now = chrono::Utc::now().timestamp() as f64;

    for (group_idx, group) in groups.iter().enumerate() {
        let group_id = format!("group_{}", group_idx);
        let shared_count = if let Some(first) = group.first() {
            group.iter().skip(1).fold(first.1.clone(), |acc, f| {
                acc.into_iter().filter(|h| f.1.contains(h)).collect()
            }).len()
        } else {
            0
        };

        for (folder_path, hashes) in group {
            let file_count = hashes.len();
            let folder_name = folder_names
                .get(folder_path)
                .cloned()
                .unwrap_or_else(|| folder_path.rsplit_once('/').map(|s| s.1.to_string()).unwrap_or_default());

            let _ = conn.execute(
                "INSERT OR REPLACE INTO duplicate_folder_groups (group_id, folder_path, folder_name, shared_count, file_count, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![group_id, folder_path, folder_name, shared_count as u64, file_count as u64, now],
            );
        }
    }

    logging::info(&format!("Duplicate folder groups refreshed: {} groups", groups.len()));
}
