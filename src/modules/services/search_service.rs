use std::io;

use crate::{
    modules::commands::command_search_file::{OrderKind, PatternKind, TargetKind},
    modules::file_entry::_types::FileEntry,
    modules::sql::{database::get_connection, search::search_file},
};

pub fn search_file_by_name(
    file_name: &str,
    target_kind: TargetKind,
    pattern_kind: PatternKind,
    order_kind: OrderKind,
) -> io::Result<Vec<FileEntry>> {
    println!("Searching for file: {}", file_name);
    let mut conn = get_connection("file_index.db").map_err(|e| {
        eprintln!("Failed to connect to database: {}", e);
        io::Error::new(io::ErrorKind::Other, e.to_string())
    })?;
    println!("Database connection established.");

    let transaction = match conn.transaction() {
        Ok(tx) => tx,
        Err(e) => {
            eprintln!("Failed to start transaction: {}", e);
            return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
        }
    };

    match search_file(
        &transaction,
        file_name,
        target_kind,
        pattern_kind,
        order_kind,
    ) {
        Ok(entries) => {
            println!("Search completed successfully.");
            println!("Found {} entries for file '{}'.", entries.len(), file_name);
            return Ok(entries);
        }
        Err(e) => {
            eprintln!("Search failed: {}", e);
            return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
        }
    };
}
