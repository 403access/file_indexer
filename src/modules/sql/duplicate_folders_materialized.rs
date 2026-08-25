use rusqlite::{Connection, Transaction};
use std::collections::{HashMap, HashSet};

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
            min_size INTEGER NOT NULL DEFAULT 0,
            updated_at REAL NOT NULL,
            UNIQUE(group_id, folder_path)
        );",
        [],
    )?;
    let _ = conn.execute(
        "ALTER TABLE duplicate_folder_groups ADD COLUMN min_size INTEGER NOT NULL DEFAULT 0",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_dfg_group_id ON duplicate_folder_groups(group_id);",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_dfg_shared_count ON duplicate_folder_groups(shared_count);",
        [],
    );
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS duplicate_folder_groups_state (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
        [],
    );
    Ok(())
}

fn get_state(conn: &Connection, key: &str) -> i64 {
    conn.query_row(
        "SELECT value FROM duplicate_folder_groups_state WHERE key = ?1",
        [key],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(0)
}

fn set_state(conn: &Connection, key: &str, value: i64) {
    let _ = conn.execute(
        "INSERT OR REPLACE INTO duplicate_folder_groups_state (key, value) VALUES (?1, ?2)",
        rusqlite::params![key, value.to_string()],
    );
}

fn max_file_id(conn: &Connection) -> i64 {
    conn.query_row("SELECT COALESCE(MAX(id), 0) FROM files", [], |r| r.get(0))
        .unwrap_or(0)
}

/// Entry point for the periodic/manual refresh.
///
/// - First run (or when the marker says so): full recompute.
/// - Subsequent runs: incremental — only folders affected by files indexed
///   since the last refresh are removed and recomputed.
pub fn refresh_duplicate_folder_groups(conn: &Connection) {
    logging::info("Refreshing duplicate folder groups...");
    let _ = create_duplicate_folder_groups_table(conn);

    let last_id = get_state(conn, "last_file_id");
    if last_id == 0 {
        full_refresh(conn);
    } else {
        incremental_refresh(conn, last_id);
    }

    set_state(conn, "last_file_id", max_file_id(conn));
}

/// Recompute the whole materialized table from scratch.
fn full_refresh(conn: &Connection) {
    let _ = conn.execute("DELETE FROM duplicate_folder_groups", []);

    let mut folder_hashes: HashMap<String, Vec<String>> = HashMap::new();
    let mut folder_names: HashMap<String, String> = HashMap::new();
    let mut folder_min_size: HashMap<String, u64> = HashMap::new();

    let mut stmt = conn
        .prepare(
            "SELECT f.path, fn.name, f.hash, f.size
             FROM files f
             JOIN file_names fn ON f.file_name_id = fn.id
             WHERE f.hash IN (SELECT hash FROM duplicate_hashes)
               AND f.hash IS NOT NULL AND f.hash != ''",
        )
        .ok();

    if let Some(ref mut s) = stmt {
        let rows = s.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        }).ok();

        if let Some(rows) = rows {
            for row in rows {
                if let Ok((path, _name, hash, size)) = row {
                    accumulate_folder(conn, &mut folder_hashes, &mut folder_names, &mut folder_min_size, &path, &hash, size);
                }
            }
        }
    }

    if folder_hashes.is_empty() {
        logging::info("No duplicate folder groups found");
        return;
    }

    let folder_list: Vec<(String, Vec<String>)> = folder_hashes.into_iter().collect();
    let groups = compute_groups(&folder_list);

    write_groups(conn, &groups, &folder_list, &folder_names, &folder_min_size, None);

    logging::info(&format!("Duplicate folder groups refreshed: {} groups", groups.len()));
}

/// Track a file's hash against its parent folder.
fn accumulate_folder(
    _conn: &Connection,
    folder_hashes: &mut HashMap<String, Vec<String>>,
    folder_names: &mut HashMap<String, String>,
    folder_min_size: &mut HashMap<String, u64>,
    path: &str,
    hash: &str,
    size: i64,
) {
    let normalized = path.replace("//", "/");
    if let Some(parent) = normalized.rsplit_once('/') {
        let folder = parent.0.to_string();
        folder_hashes.entry(folder.clone()).or_default().push(hash.to_string());
        let size = size.max(0) as u64;
        folder_min_size
            .entry(folder.clone())
            .and_modify(|m| *m = (*m).min(size))
            .or_insert(size);
        if let Some(display) = folder.rsplit_once('/') {
            folder_names.entry(folder.clone()).or_insert_with(|| display.1.to_string());
        }
    }
}

/// Union-find over folders sharing at least one duplicate hash.
/// Returns the list of group partitions where each group has ≥2 folders.
fn compute_groups(folder_list: &[(String, Vec<String>)]) -> Vec<Vec<usize>> {
    let hash_to_folders: HashMap<&str, Vec<usize>> = {
        let mut idx: HashMap<&str, Vec<usize>> = HashMap::new();
        for (i, (_, hashes)) in folder_list.iter().enumerate() {
            for h in hashes {
                idx.entry(h.as_str()).or_default().push(i);
            }
        }
        idx
    };

    let mut parent: Vec<usize> = (0..folder_list.len()).collect();
    fn find(parent: &mut Vec<usize>, x: usize) -> usize {
        if parent[x] == x {
            return x;
        }
        parent[x] = find(parent, parent[x]);
        parent[x]
    }
    fn union(parent: &mut Vec<usize>, a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent[rb] = ra;
        }
    }

    for folders in hash_to_folders.values() {
        if folders.len() < 2 {
            continue;
        }
        for k in 1..folders.len() {
            union(&mut parent, folders[0], folders[k]);
        }
    }

    let mut grouped: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, _) in folder_list.iter().enumerate() {
        let root = find(&mut parent, i);
        grouped.entry(root).or_default().push(i);
    }

    let mut groups: Vec<Vec<usize>> = grouped.into_values().collect();
    groups.retain(|g| g.len() >= 2);
    groups
}

/// Insert one row per (group, folder) using the current table schema.
fn write_groups(
    conn: &Connection,
    groups: &[Vec<usize>],
    folder_list: &[(String, Vec<String>)],
    folder_names: &HashMap<String, String>,
    folder_min_size: &HashMap<String, u64>,
    tx: Option<&Transaction>,
) {
    let now = chrono::Utc::now().timestamp() as f64;
    let mut group_start = next_group_number(conn);

    for group in groups {
        let group_id = format!("group_{}", group_start);
        group_start += 1;

        let mut hash_occurrences: HashMap<&str, usize> = HashMap::new();
        for fi in group {
            for h in folder_list[*fi].1.iter() {
                *hash_occurrences.entry(h.as_str()).or_insert(0) += 1;
            }
        }
        let shared_count = hash_occurrences
            .iter()
            .filter(|(_, n)| **n == group.len())
            .count();

        for fi in group {
            let (folder_path, hashes) = &folder_list[*fi];
            let file_count = hashes.len();
            let folder_name = folder_names
                .get(folder_path)
                .cloned()
                .unwrap_or_else(|| folder_path.rsplit_once('/').map(|s| s.1.to_string()).unwrap_or_default());
            let min_size = folder_min_size.get(folder_path).copied().unwrap_or(0);

            let stmt = "INSERT OR REPLACE INTO duplicate_folder_groups
                 (group_id, folder_path, folder_name, shared_count, file_count, min_size, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";
            let params = rusqlite::params![group_id, folder_path, folder_name, shared_count as u64, file_count as u64, min_size, now];

            let res = match tx {
                Some(t) => t.execute(stmt, params),
                None => conn.execute(stmt, params),
            };
            if let Err(e) = res {
                logging::error(&format!("Failed to insert duplicate folder group '{}' ({}): {}", folder_path, group_id, e));
            }
        }
    }
}

/// Highest numeric suffix used in existing group ids (for naming new groups).
fn next_group_number(conn: &Connection) -> u32 {
    let max: Option<String> = conn
        .query_row("SELECT MAX(group_id) FROM duplicate_folder_groups", [], |r| r.get(0))
        .unwrap_or(None);
    match max {
        Some(_) => {
            let mut max_n = 0u32;
            let mut stmt = conn
                .prepare("SELECT group_id FROM duplicate_folder_groups")
                .ok();
            if let Some(ref mut s) = stmt {
                if let Ok(rows) = s.query_map([], |row| row.get::<_, String>(0)) {
                    for row in rows.flatten() {
                        let n = row
                            .trim_start_matches("group_")
                            .parse::<u32>()
                            .unwrap_or(0);
                        max_n = max_n.max(n);
                    }
                }
            }
            max_n + 1
        }
        None => 0,
    }
}

/// Recompute only the folder groups affected by files indexed since `from_file_id`.
fn incremental_refresh(conn: &Connection, from_file_id: i64) {
    // Newly indexed files that turned out to be duplicates.
    let affected: HashSet<String> = {
        let mut stmt = conn
            .prepare(
                "SELECT f.path
                 FROM files f
                 WHERE f.id > ?1
                   AND f.hash IS NOT NULL AND f.hash != ''
                   AND f.hash IN (SELECT hash FROM duplicate_hashes)",
            )
            .ok();
        let mut set = HashSet::new();
        if let Some(ref mut s) = stmt {
            if let Ok(rows) = s.query_map(rusqlite::params![from_file_id], |r| r.get::<_, String>(0)) {
                for row in rows.flatten() {
                    if let Some(parent) = row.replace("//", "/").rsplit_once('/') {
                        set.insert(parent.0.to_string());
                    }
                }
            }
        }
        set
    };

    if affected.is_empty() {
        logging::info("Incremental duplicate folder groups update: no new changes");
        return;
    }

    // Expand to the whole connected component(s): any folder sharing a hash
    // with an affected folder must be re-evaluated too.
    let affected_closure = grow_closure(conn, &affected);

    // Read folder -> (hashes, min_size, name) for the affected folders only.
    let mut folder_hashes: HashMap<String, Vec<String>> = HashMap::new();
    let mut folder_names: HashMap<String, String> = HashMap::new();
    let mut folder_min_size: HashMap<String, u64> = HashMap::new();

    let _ = conn.execute(
        "CREATE TEMP TABLE dfg_incremental_scope (folder_path TEXT PRIMARY KEY)",
        [],
    );
    for folder in &affected_closure {
        let _ = conn.execute(
            "INSERT OR IGNORE INTO dfg_incremental_scope (folder_path) VALUES (?1)",
            [folder],
        );
    }

    let mut stmt = conn
        .prepare(
            "SELECT f.path, fn.name, f.hash, f.size
             FROM files f
             JOIN file_names fn ON f.file_name_id = fn.id
             WHERE f.hash IN (SELECT hash FROM duplicate_hashes)
               AND f.hash IS NOT NULL AND f.hash != ''
               AND f.parent_path IN (SELECT folder_path FROM dfg_incremental_scope)",
        )
        .ok();

    if let Some(ref mut s) = stmt {
        let rows = s.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        }).ok();

        if let Some(rows) = rows {
            for row in rows {
                if let Ok((path, _name, hash, size)) = row {
                    accumulate_folder(conn, &mut folder_hashes, &mut folder_names, &mut folder_min_size, &path, &hash, size);
                }
            }
        }
    }
    let _ = conn.execute("DROP TABLE IF EXISTS dfg_incremental_scope", []);

    if folder_hashes.is_empty() {
        logging::info("Incremental duplicate folder groups update: affected folders have no groups");
        return;
    }

    // Remove the affected folders (and only them) from the materialized table.
    {
        let placeholders = vec!["?"; affected_closure.len()].join(",");
        let mut sql = format!("DELETE FROM duplicate_folder_groups WHERE folder_path IN ({placeholders})");
        if affected_closure.is_empty() {
            sql = "DELETE FROM duplicate_folder_groups WHERE 0".to_string();
        }
        if let Ok(mut stmt) = conn.prepare(&sql) {
            let params: Vec<&str> = affected_closure.iter().map(|s| s.as_str()).collect();
            let _ = stmt.execute(rusqlite::params_from_iter(params));
        }
    }

    let folder_list: Vec<(String, Vec<String>)> = folder_hashes.into_iter().collect();
    let groups = compute_groups(&folder_list);

    let tx = conn.unchecked_transaction().ok();
    write_groups(conn, &groups, &folder_list, &folder_names, &folder_min_size, tx.as_ref());
    if let Some(t) = tx {
        let _ = t.commit();
    }

    logging::info(&format!(
        "Incremental duplicate folder groups update: {} affected folder(s), {} group(s) recomputed",
        affected_closure.len(),
        groups.len()
    ));
}

/// From a seed set of folders, expand to every folder sharing a duplicate hash
/// (transitively) so connected components stay intact.
fn grow_closure(conn: &Connection, seed: &HashSet<String>) -> HashSet<String> {
    let mut closure: HashSet<String> = seed.clone();
    let mut frontier: HashSet<String> = seed.iter().cloned().collect();

    while !frontier.is_empty() {
        let prev_len = closure.len();

        // Hashes present in the current frontier folders.
        let mut hashes: HashSet<String> = HashSet::new();
        {
            let placeholders = vec!["?"; frontier.len()].join(",");
            let sql = format!(
                "SELECT DISTINCT f.hash FROM files f
                 WHERE f.parent_path IN ({placeholders})
                   AND f.hash IS NOT NULL AND f.hash != ''
                   AND f.hash IN (SELECT hash FROM duplicate_hashes)"
            );
            if let Ok(mut stmt) = conn.prepare(&sql) {
                let params: Vec<&str> = frontier.iter().map(|s| s.as_str()).collect();
                if let Ok(rows) = stmt.query_map(rusqlite::params_from_iter(params), |r| r.get::<_, String>(0)) {
                    for row in rows.flatten() {
                        hashes.insert(row);
                    }
                }
            }
        }

        // Folders holding any of those hashes.
        let mut new_folders: HashSet<String> = HashSet::new();
        if !hashes.is_empty() {
            let chunks: Vec<Vec<&str>> = hashes
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .chunks(500)
                .map(|c| c.to_vec())
                .collect();
            for chunk in chunks {
                let placeholders = vec!["?"; chunk.len()].join(",");
                let sql = format!(
                    "SELECT DISTINCT f.parent_path FROM files f
                     WHERE f.hash IN ({placeholders})
                       AND f.hash IS NOT NULL AND f.hash != ''
                       AND f.parent_path IS NOT NULL"
                );
                if let Ok(mut stmt) = conn.prepare(&sql) {
                    let ids: Vec<&str> = chunk;
                    if let Ok(rows) = stmt.query_map(rusqlite::params_from_iter(ids), |r| r.get::<_, String>(0)) {
                        for row in rows.flatten() {
                            new_folders.insert(row);
                        }
                    }
                }
            }
        }

        frontier = new_folders.difference(&closure).cloned().collect();
        closure.extend(new_folders);

        if closure.len() == prev_len {
            break;
        }
    }

    closure
}