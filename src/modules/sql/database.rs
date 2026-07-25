use rusqlite::{named_params, Connection, Transaction};

use crate::modules::file_entry::_types::FileEntry;

pub fn get_connection(db_path: &str) -> rusqlite::Result<Connection> {
    Connection::open(db_path)
}

pub fn init_db(tx: &Transaction) -> rusqlite::Result<()> {
    let file_names_table_result = tx.execute(
        "CREATE TABLE IF NOT EXISTS file_names (
            id INTEGER PRIMARY KEY,
            name TEXT UNIQUE
        );",
        [],
    );

    if file_names_table_result.is_err() {
        eprintln!(
            "Failed to create 'file_names' table: {:?}",
            file_names_table_result
        );
        return Err(file_names_table_result.unwrap_err());
    }

    let files_table_result = tx.execute(
        "CREATE TABLE IF NOT EXISTS files (
            id INTEGER PRIMARY KEY,
            path TEXT UNIQUE,
            file_name_id INTEGER,
            size INTEGER,
            modified REAL,
            hash TEXT,
            is_directory INTEGER,
            is_file INTEGER,
            is_symlink INTEGER,

            FOREIGN KEY(file_name_id) REFERENCES file_names(id)
        );",
        [],
    );

    if files_table_result.is_err() {
        eprintln!("Failed to create 'files' table: {:?}", files_table_result);
        return Err(files_table_result.unwrap_err());
    }

    let hash_index_result = tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_files_hash ON files(hash);",
        [],
    );
    if hash_index_result.is_err() {
        eprintln!("Failed to create hash index: {:?}", hash_index_result);
        return Err(hash_index_result.unwrap_err());
    }

    let path_index_result = tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);",
        [],
    );
    if path_index_result.is_err() {
        eprintln!("Failed to create path index: {:?}", path_index_result);
        return Err(path_index_result.unwrap_err());
    }

    return Ok(());
}

pub fn insert_file_name(tx: &Transaction, name: &str) -> rusqlite::Result<i64> {
    let affected = tx.execute(
        "INSERT OR IGNORE INTO file_names (name) VALUES (:name)",
        named_params! { ":name": name },
    )?;
    if affected == 1 {
        // Row was inserted, return the new id
        Ok(tx.last_insert_rowid())
    } else if affected == 0 {
        // Row was ignored (already exists)
        Err(rusqlite::Error::ExecuteReturnedResults)
    } else {
        Err(rusqlite::Error::ExecuteReturnedResults)
    }
}

/// We loop over the entries and insert them into the database in a transaction.
/// rusqlite does not support bulk inserts directly, so we insert each file individually.
pub fn insert_file(tx: &Transaction, file: &FileEntry, file_name_id: i64) -> rusqlite::Result<i64> {
    let affected = tx.execute(
        "INSERT OR IGNORE INTO files (path, file_name_id, size, modified, hash, is_directory, is_file, is_symlink)
         VALUES (:path, :name_id, :size, :modified, :hash, :is_directory, :is_file, :is_symlink)",
        named_params! {
            ":path": &file.path,
            ":name_id": file_name_id,
            ":size": &file.size,
            ":modified": &file.modified,
            ":hash": &file.hash,
            ":is_directory": file.is_directory as i32,
            ":is_file": file.is_file as i32,
            ":is_symlink": file.is_symlink as i32,
        },
    )?;
    if affected == 1 {
        // Row was inserted, return the new id
        Ok(tx.last_insert_rowid())
    } else if affected == 0 {
        // Row was ignored (already exists)
        Err(rusqlite::Error::ExecuteReturnedResults)
    } else {
        Err(rusqlite::Error::ExecuteReturnedResults)
    }
}

pub fn create_duplicates_table(tx: &Transaction) -> rusqlite::Result<()> {
    tx.execute(
        "
        CREATE TABLE IF NOT EXISTS duplicate_hashes AS
        SELECT hash
        FROM files
        GROUP BY hash
        HAVING COUNT(*) > 1;
        ",
        [],
    )?;
    Ok(())
}

pub fn remove_duplicates_table(tx: &Transaction) -> rusqlite::Result<()> {
    tx.execute("DROP TABLE IF EXISTS duplicate_hashes", [])?;
    Ok(())
}

pub fn reset_duplicates_table(tx: &Transaction) -> rusqlite::Result<()> {
    remove_duplicates_table(tx)?;
    create_duplicates_table(tx)?;
    Ok(())
}
