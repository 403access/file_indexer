use rusqlite::{Connection, Transaction};

use crate::modules::file_entry::{_types::FileEntry, convert::convert_from_rows};
use crate::modules::logging;

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

/// Refresh the duplicate_hashes table (drop + recreate). Call after folder commits.
pub fn refresh_duplicate_hashes(conn: &Connection) {
    logging::info("Refreshing duplicate_hashes table...");
    let _ = conn.execute("DROP TABLE IF EXISTS duplicate_hashes", []);
    let _ = conn.execute(
        "CREATE TABLE duplicate_hashes AS
         SELECT hash FROM files
         WHERE hash IS NOT NULL AND hash != ''
         GROUP BY hash HAVING COUNT(*) > 1",
        [],
    );
    let count: u64 = conn
        .query_row("SELECT COUNT(*) FROM duplicate_hashes", [], |r| r.get(0))
        .unwrap_or(0);
    logging::info(&format!("Duplicate hashes refreshed: {} duplicate groups", count));
}

/// Incrementally update duplicate_hashes for only the given hashes.
/// Checks if each hash is now a duplicate (count > 1) and adds it if so.
pub fn update_duplicate_hashes_incremental(conn: &Connection, hashes: &[String]) {
    if hashes.is_empty() {
        return;
    }

    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS duplicate_hashes (hash TEXT PRIMARY KEY)",
        [],
    );

    let mut new_duplicates = 0u64;

    for hash in hashes {
        let already_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM duplicate_hashes WHERE hash = ?1",
                [hash],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if already_exists {
            continue;
        }

        let count: u64 = conn
            .query_row(
                "SELECT COUNT(*) FROM files WHERE hash = ?1 AND hash IS NOT NULL AND hash != ''",
                [hash],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if count > 1 {
            let _ = conn.execute(
                "INSERT OR IGNORE INTO duplicate_hashes (hash) VALUES (?1)",
                [hash],
            );
            new_duplicates += 1;
        }
    }

    if new_duplicates > 0 {
        logging::info(&format!("Incremental duplicate update: {} new duplicate group(s) found from {} hash(es)", new_duplicates, hashes.len()));
    }
}

pub fn get_duplicates(tx: &Transaction, limit: Option<u64>) -> rusqlite::Result<Vec<FileEntry>> {
    reset_duplicates_table(tx)?;

    let limit = limit.unwrap_or(100);
    let sql = format!(
        "
        SELECT f.path, fn.name, f.size, f.modified, f.hash,
               f.is_directory, f.is_file, f.is_symlink, f.parent_path
        FROM files f
        JOIN file_names fn ON f.file_name_id = fn.id
        JOIN duplicate_hashes d ON f.hash = d.hash
        LIMIT {}
        ",
        limit
    );
    let mut stmt = tx.prepare(&sql)?;

    println!("[info] Getting duplicates...");
    let mut rows = stmt.query(rusqlite::params![])?;

    convert_from_rows(&mut rows)
}
