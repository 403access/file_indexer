use rusqlite::Connection;
use std::collections::HashMap;

use crate::modules::logging;
use crate::modules::near_duplicates::{
    find_near_duplicate_pairs, hash_to_token, is_noise_file, FolderSet, NearDupConfig,
    NearDupPair,
};

/// Near-duplicate configuration resolved from env vars.
#[derive(Debug, Clone)]
pub struct RefreshParams {
    pub min_similarity: f64,
    pub num_perm: u32,
    pub bands: u32,
}

impl Default for RefreshParams {
    fn default() -> Self {
        Self {
            min_similarity: 0.8,
            num_perm: 64,
            bands: 8,
        }
    }
}

pub fn create_near_duplicate_tables(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS near_duplicate_folder_pairs (
            id INTEGER PRIMARY KEY,
            folder_a TEXT NOT NULL,
            folder_b TEXT NOT NULL,
            similarity REAL NOT NULL,
            shared_files INTEGER NOT NULL,
            union_files INTEGER NOT NULL,
            updated_at REAL NOT NULL,
            UNIQUE(folder_a, folder_b)
        );",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_ndfp_similarity ON near_duplicate_folder_pairs(similarity);",
        [],
    )?;
    Ok(())
}

/// Recompute the near-duplicate pairs table from scratch.
///
/// Loads every indexed file's content hash grouped by parent folder, filters
/// noise/empty files, runs the MinHash+LSH pipeline and materializes the
/// verified pairs. Returns the number of stored pairs.
pub fn refresh_near_duplicate_folder_pairs(
    conn: &mut Connection,
    params: &RefreshParams,
) -> usize {
    logging::info("Refreshing near-duplicate folder pairs...");
    if let Err(e) = create_near_duplicate_tables(conn) {
        logging::error(&format!("Failed to create near-duplicate tables: {}", e));
        return 0;
    }

    let folders = load_folder_sets(conn);
    let cfg = NearDupConfig {
        min_similarity: params.min_similarity,
        num_perm: params.num_perm,
        bands: params.bands,
        ..Default::default()
    };
    let pairs = find_near_duplicate_pairs(&folders, &cfg);

    match store_pairs(conn, &folders, &pairs) {
        Ok(count) => {
            logging::info(&format!(
                "Near-duplicate folder pairs refreshed: {} folders scanned, {} pairs stored",
                folders.len(),
                count
            ));
            count
        }
        Err(e) => {
            logging::error(&format!("Failed to store near-duplicate pairs: {}", e));
            0
        }
    }
}

fn load_folder_sets(conn: &Connection) -> Vec<FolderSet> {
    let mut stmt = match conn.prepare(
        "SELECT f.path, f.parent_path, fn.name, f.hash
         FROM files f
         JOIN file_names fn ON f.file_name_id = fn.id
         WHERE f.is_file = 1
           AND f.hash IS NOT NULL AND f.hash != ''
           AND f.size > 0",
    ) {
        Ok(s) => s,
        Err(e) => {
            logging::error(&format!("Failed to query files for near-dup scan: {}", e));
            return Vec::new();
        }
    };

    struct RawFolder {
        name: String,
        tokens: Vec<u64>,
    }
    // path without trailing slash → accumulated token list
    let mut by_folder: HashMap<String, RawFolder> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    });

    let rows = match rows {
        Ok(r) => r,
        Err(e) => {
            logging::error(&format!("Failed to read file rows for near-dup scan: {}", e));
            return Vec::new();
        }
    };

    for row in rows.flatten() {
        let (path, parent_path, name, hash) = row;
        if is_noise_file(&name) {
            continue;
        }
        let folder = parent_path.unwrap_or_else(|| {
            path.rsplit_once('/').map(|(p, _)| p.to_string()).unwrap_or(path.clone())
        });
        let folder_name = folder
            .rsplit_once('/')
            .map(|(_, n)| n.to_string())
            .unwrap_or_else(|| folder.clone());
        let entry = by_folder.entry(folder.clone()).or_insert_with(|| RawFolder {
            name: folder_name,
            tokens: Vec::new(),
        });
        if entry.name.is_empty() {
            entry.name = folder
                .rsplit_once('/')
                .map(|(_, n)| n.to_string())
                .unwrap_or(folder);
        }
        entry.tokens.push(hash_to_token(&hash));
    }

    by_folder
        .into_iter()
        .map(|(path, raw)| FolderSet::new(path, raw.name, raw.tokens))
        .collect()
}

fn store_pairs(
    conn: &mut Connection,
    folders: &[FolderSet],
    pairs: &[NearDupPair],
) -> rusqlite::Result<usize> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM near_duplicate_folder_pairs", [])?;
    let now = chrono::Utc::now().timestamp() as f64;
    for p in pairs {
        tx.execute(
            "INSERT OR IGNORE INTO near_duplicate_folder_pairs
                (folder_a, folder_b, similarity, shared_files, union_files, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                folders[p.folder_a].path,
                folders[p.folder_b].path,
                p.similarity,
                p.intersection as i64,
                p.union as i64,
                now
            ],
        )?;
    }
    let count = tx
        .query_row("SELECT COUNT(*) FROM near_duplicate_folder_pairs", [], |r| {
            r.get::<_, i64>(0)
        })?;
    tx.commit()?;
    Ok(count as usize)
}

#[derive(Debug, serde::Serialize)]
pub struct DeltaFile {
    pub name: String,
    pub size: i64,
    pub hash: String,
}

#[derive(Debug, serde::Serialize)]
pub struct PairDelta {
    pub folder_a: String,
    pub folder_b: String,
    /// Files present in A but missing in B (by name).
    pub only_in_a: Vec<DeltaFile>,
    /// Files present in B but missing in A (by name).
    pub only_in_b: Vec<DeltaFile>,
    /// Same name, different content hash.
    pub changed: Vec<DeltaFile>,
    /// Same name and identical content hash.
    pub identical_count: usize,
}

fn load_folder_files(conn: &Connection, folder: &str) -> HashMap<String, (i64, String)> {
    let mut map = HashMap::new();
    let mut stmt = match conn.prepare(
        "SELECT fn.name, f.size, COALESCE(f.hash, '')
         FROM files f
         JOIN file_names fn ON f.file_name_id = fn.id
         WHERE f.parent_path = ?1 AND f.is_file = 1",
    ) {
        Ok(s) => s,
        Err(_) => return map,
    };
    if let Ok(rows) = stmt.query_map([folder], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
        ))
    }) {
        for row in rows.flatten() {
            map.insert(row.0, (row.1, row.2));
        }
    }
    map
}

/// Compute the concrete file-level differences between two folders.
///
/// Comparison is by file name within the folder: entries present under the
/// same name with the same hash count as identical, same name + different
/// hash as changed, otherwise reported as one-sided additions/removals.
pub fn compute_pair_delta(conn: &Connection, folder_a: &str, folder_b: &str) -> PairDelta {
    let fa = load_folder_files(conn, folder_a);
    let fb = load_folder_files(conn, folder_b);

    let mut delta = PairDelta {
        folder_a: folder_a.to_string(),
        folder_b: folder_b.to_string(),
        only_in_a: Vec::new(),
        only_in_b: Vec::new(),
        changed: Vec::new(),
        identical_count: 0,
    };

    for (name, (size, hash)) in &fa {
        match fb.get(name) {
            None => delta.only_in_a.push(DeltaFile {
                name: name.clone(),
                size: *size,
                hash: hash.clone(),
            }),
            Some((_b_size, b_hash)) if b_hash == hash => delta.identical_count += 1,
            Some((b_size, b_hash)) => delta.changed.push(DeltaFile {
                name: name.clone(),
                size: (*size).max(*b_size),
                hash: format!("{hash} → {b_hash}"),
            }),
        }
    }
    for (name, (size, hash)) in &fb {
        if !fa.contains_key(name) {
            delta.only_in_b.push(DeltaFile {
                name: name.clone(),
                size: *size,
                hash: hash.clone(),
            });
        }
    }

    delta.only_in_a.sort_by(|a, b| a.name.cmp(&b.name));
    delta.only_in_b.sort_by(|a, b| a.name.cmp(&b.name));
    delta.changed.sort_by(|a, b| a.name.cmp(&b.name));
    delta
}
