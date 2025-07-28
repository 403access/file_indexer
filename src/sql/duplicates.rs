use rusqlite::Transaction;

use crate::file_entry::{_types::FileEntry, convert::convert_from_rows};

pub fn get_duplicates(tx: &Transaction, limit: Option<u64>) -> rusqlite::Result<Vec<FileEntry>> {
    let limit = limit.unwrap_or(100);
    let sql = format!(
        "
        SELECT f.*
        FROM files f
        JOIN duplicate_hashes d ON f.hash = d.hash
        LIMIT {}
        ",
        limit
    );
    let mut stmt = tx.prepare(&sql)?;

    let mut rows = stmt.query(rusqlite::params![])?;
    // Simulate getting duplicates
    println!("[info] Getting duplicates...");

    convert_from_rows(&mut rows)
}
