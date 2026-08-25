use std::collections::HashSet;

use rusqlite::{named_params, Connection, OptionalExtension, Transaction};

use crate::modules::file_entry::_types::FileEntry;

pub fn insert_file_name(tx: &Transaction, name: &str) -> rusqlite::Result<i64> {
    let affected = tx.execute(
        "INSERT OR IGNORE INTO file_names (name) VALUES (:name)",
        named_params! { ":name": name },
    )?;
    if affected == 1 {
        Ok(tx.last_insert_rowid())
    } else if affected == 0 {
        Err(rusqlite::Error::ExecuteReturnedResults)
    } else {
        Err(rusqlite::Error::ExecuteReturnedResults)
    }
}

pub fn get_or_insert_file_name(tx: &Transaction, name: &str) -> rusqlite::Result<i64> {
    tx.execute(
        "INSERT OR IGNORE INTO file_names (name) VALUES (:name)",
        named_params! { ":name": name },
    )?;
    tx.query_row(
        "SELECT id FROM file_names WHERE name = :name",
        named_params! { ":name": name },
        |row| row.get(0),
    )
}

/// We loop over the entries and insert them into the database in a transaction.
/// rusqlite does not support bulk inserts directly, so we insert each file individually.
pub fn insert_file(tx: &Transaction, file: &FileEntry, file_name_id: i64, parent_path: Option<&str>) -> rusqlite::Result<i64> {
    let affected = tx.execute(
        "INSERT OR IGNORE INTO files (path, file_name_id, size, modified, hash, is_directory, is_file, is_symlink, parent_path)
         VALUES (:path, :name_id, :size, :modified, :hash, :is_directory, :is_file, :is_symlink, :parent_path)",
        named_params! {
            ":path": &file.path,
            ":name_id": file_name_id,
            ":size": &file.size,
            ":modified": &file.modified,
            ":hash": &file.hash,
            ":is_directory": file.is_directory as i32,
            ":is_file": file.is_file as i32,
            ":is_symlink": file.is_symlink as i32,
            ":parent_path": parent_path,
        },
    )?;
    if affected == 1 {
        Ok(tx.last_insert_rowid())
    } else {
        Ok(0)
    }
}

pub fn mark_directory_traversed(tx: &Transaction, path: &str) -> rusqlite::Result<()> {
    tx.execute(
        "UPDATE files SET traversed = 1 WHERE path = ?1",
        [path],
    )?;
    Ok(())
}

/// Store an aggregate size for a directory row (used for leaf folders whose
/// size = sum of the sizes of the files they contain directly).
pub fn set_directory_size(tx: &Transaction, path: &str, size: u64) -> rusqlite::Result<()> {
    tx.execute(
        "UPDATE files SET size = ?1 WHERE path = ?2 AND is_directory = 1",
        rusqlite::params![size, path],
    )?;
    Ok(())
}

/// Set a folder as fully indexed AND refresh its stored mtime.
/// The mtime is what later runs compare against to decide whether a
/// reconcile is needed. Any prior traversal error is cleared.
pub fn mark_directory_traversed_and_modified(
    tx: &Transaction,
    path: &str,
    modified: Option<f64>,
) -> rusqlite::Result<()> {
    tx.execute(
        "UPDATE files SET traversed = 1, modified = ?1, traverse_error = NULL WHERE path = ?2 AND is_directory = 1",
        rusqlite::params![modified, path],
    )?;
    Ok(())
}

/// Mark a folder as attempted-but-unreadable, recording the error and the
/// disk mtime seen at attempt time so later runs can skip it while unchanged.
pub fn mark_directory_error(
    tx: &Transaction,
    path: &str,
    error: &str,
    modified: Option<f64>,
) -> rusqlite::Result<()> {
    tx.execute(
        "UPDATE files SET traversed = 0, traverse_error = ?1, modified = ?2 WHERE path = ?3 AND is_directory = 1",
        rusqlite::params![error, modified, path],
    )?;
    Ok(())
}

pub struct DirectoryMetadata {
    pub traversed: bool,
    pub modified: Option<f64>,
    pub traverse_error: Option<String>,
}

pub fn is_directory_indexed(conn: &Connection, path: &str) -> bool {
    let trimmed = path.trim_end_matches('/');
    conn.query_row(
        "SELECT traversed FROM files WHERE path = ?1 AND is_directory = 1",
        [trimmed],
        |row| row.get::<_, i32>(0),
    )
    .unwrap_or(0)
        == 1
}

/// Stored traversal state + mtime for a directory, so a re-run can decide
/// whether its children need to be re-read from disk, or whether a previous
/// read failed (`traverse_error`).
pub fn get_directory_metadata(
    conn: &Connection,
    path: &str,
) -> rusqlite::Result<Option<DirectoryMetadata>> {
    let trimmed = path.trim_end_matches('/');
    conn.query_row(
        "SELECT traversed, modified, traverse_error FROM files WHERE path = ?1 AND is_directory = 1",
        [trimmed],
        |row| {
            let traverse_error: Option<String> = row.get(2)?;
            Ok(DirectoryMetadata {
                traversed: row.get::<_, i32>(0)? != 0,
                modified: row.get(1)?,
                traverse_error,
            })
        },
    )
    .optional()
}

/// Child rows (path + is_directory) of a folder. Includes files and folders.
pub fn get_child_entries(conn: &Connection, parent_path: &str) -> rusqlite::Result<Vec<(String, bool)>> {
    let mut stmt = conn.prepare(
        "SELECT path, is_directory FROM files WHERE parent_path = ?1",
    )?;
    let rows = stmt.query_map([parent_path], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i32>(1)? != 0,
        ))
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn get_child_directories(conn: &Connection, parent_path: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT path FROM files WHERE parent_path = ?1 AND is_directory = 1",
    )?;
    let rows = stmt.query_map([parent_path], |row| row.get(0))?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub struct UpsertResult {
    /// Whether a brand-new row was inserted.
    pub inserted: bool,
    /// Whether an existing row's hash was changed by this write.
    pub hash_updated: bool,
}

/// Insert a fresh row or update the existing one (matched by unique `path`).
/// Unlike `insert_file`, this refreshes size/modified/hash so a re-scan of a
/// changed folder reflects new content. `traversed` is left untouched here so
/// a re-scan doesn't silently re-flag child folders.
pub fn upsert_file(
    tx: &Transaction,
    file: &FileEntry,
    file_name_id: i64,
    parent_path: Option<&str>,
) -> rusqlite::Result<UpsertResult> {
    let path = file.path.as_deref().unwrap_or("");
    let existing_hash: Option<Option<String>> = tx
        .query_row(
            "SELECT hash FROM files WHERE path = ?1",
            [path],
            |row| row.get(0),
        )
        .optional()?;

    match existing_hash {
        None => {
            insert_file(tx, file, file_name_id, parent_path)?;
            Ok(UpsertResult {
                inserted: true,
                hash_updated: false,
            })
        }
        Some(prev_hash) => {
            tx.execute(
                "UPDATE files SET file_name_id = :name_id, size = :size, modified = :modified,
                         hash = :hash, is_directory = :is_directory, is_file = :is_file,
                         is_symlink = :is_symlink, parent_path = :parent_path
                 WHERE path = :path",
                named_params! {
                    ":name_id": file_name_id,
                    ":size": &file.size,
                    ":modified": &file.modified,
                    ":hash": &file.hash,
                    ":is_directory": file.is_directory as i32,
                    ":is_file": file.is_file as i32,
                    ":is_symlink": file.is_symlink as i32,
                    ":parent_path": parent_path,
                    ":path": path,
                },
            )?;
            let hash_updated = !file.is_directory
                && file.hash.is_some()
                && file.hash != prev_hash;
            Ok(UpsertResult {
                inserted: false,
                hash_updated,
            })
        }
    }
}

/// Delete rows (and their whole subtrees for folders) that are still in the DB
/// under `parent_path` but no longer exist on disk.
pub fn delete_stale_children(
    tx: &Transaction,
    parent_path: &str,
    keep_paths: &HashSet<String>,
) -> rusqlite::Result<usize> {
    let mut removed = 0usize;
    for (child, is_dir) in get_child_entries(tx, parent_path)? {
        if keep_paths.contains(&child) {
            continue;
        }
        if is_dir {
            removed += delete_directory_tree(tx, &child)?;
        } else {
            removed += tx.execute("DELETE FROM files WHERE path = ?1", [&child])?;
        }
    }
    Ok(removed)
}

/// Remove a directory and all of its descendants from the DB.
/// Deletes deepest rows first so `parent_path` FK references are never broken.
pub fn delete_directory_tree(tx: &Transaction, path: &str) -> rusqlite::Result<usize> {
    let mut stmt = tx.prepare(
        "WITH RECURSIVE subtree(p) AS (
            SELECT ?1
            UNION ALL
            SELECT f.path
            FROM files f INDEXED BY idx_files_parent_path
            JOIN subtree s ON f.parent_path = s.p
         )
         SELECT p FROM subtree",
    )?;
    let rows = stmt.query_map([path], |row| row.get::<_, String>(0))?;
    let mut to_delete: Vec<String> = Vec::new();
    for row in rows {
        to_delete.push(row?);
    }
    // Delete deepest paths first so child folders are gone before their parents.
    to_delete.sort_by_key(|p| std::cmp::Reverse(p.matches('/').count()));
    let mut removed = 0usize;
    for p in &to_delete {
        removed += tx.execute("DELETE FROM files WHERE path = ?1", [p])?;
    }
    Ok(removed)
}
