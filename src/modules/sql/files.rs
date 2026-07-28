use rusqlite::{named_params, Connection, Transaction};

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
