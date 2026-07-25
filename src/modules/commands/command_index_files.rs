use std::io;

use indicatif::ProgressBar;

use crate::{
    modules::{
        file_entry::_types::FileEntry,
        search_files::try_get_dir_entries::try_get_dir_entries,
        sql::database::{get_connection, init_db, insert_file, insert_file_name},
    },
    states::app_state,
};

/// Core indexing function. Recursively scans `root_dir` and stores all files
/// and directories into the SQLite database at `db_path`.
pub fn index_directory(db_path: &str, root_dir: &str) -> io::Result<()> {
    let mut names: Vec<(String, i64)> = vec![];

    let mut conn = get_connection(db_path).map_err(|e| {
        eprintln!("Failed to connect to database: {}", e);
        io::Error::new(io::ErrorKind::Other, e.to_string())
    })?;

    {
        let tx = conn.transaction().map_err(|e| {
            eprintln!("Failed to start transaction: {}", e);
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
        init_db(&tx).map_err(|e| {
            eprintln!("Failed to initialize database: {}", e);
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
        tx.commit().map_err(|e| {
            eprintln!("Failed to commit transaction: {}", e);
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
    }

    let root = if root_dir.ends_with('/') {
        root_dir.to_string()
    } else {
        format!("{}/", root_dir)
    };

    let mut paths: Vec<String> = vec![root];
    loop {
        let transaction = match conn.transaction() {
            Ok(tx) => tx,
            Err(e) => {
                eprintln!("Failed to start transaction: {}", e);
                return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
            }
        };

        let mut new_paths: Vec<String> = Vec::new();
        for path in &paths {
            match get_and_insert_entries(&transaction, &mut names, path) {
                Ok(dirs) => {
                    new_paths.extend(
                        dirs.iter()
                            .filter(|d| d.is_directory)
                            .map(|d| format!("{}/{}/", path, d.name)),
                    );
                }
                Err(e) => {
                    eprintln!("Skipping unreadable directory '{}': {}", path, e);
                }
            }
        }

        transaction.commit().map_err(|e| {
            eprintln!("Failed to commit transaction: {}", e);
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;

        if new_paths.is_empty() {
            break;
        }

        paths.clear();
        paths.extend(new_paths);
    }
    Ok(())
}

pub fn command_index_files(_pb: &ProgressBar) -> io::Result<bool> {
    println!("Starting directory listing...");

    let cwd = app_state::get_cwd();
    let db_path = "file_index.db";

    index_directory(db_path, &cwd)?;

    println!("Indexing complete.");
    Ok(false)
}

fn get_and_insert_entries(
    transaction: &rusqlite::Transaction,
    names: &mut Vec<(String, i64)>,
    path: &String,
) -> io::Result<Vec<FileEntry>> {
    let entries = match try_get_dir_entries(&path, None) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Failed to retrieve directory entries: {}", e);
            return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
        }
    };

    let mut directories: Vec<FileEntry> = Vec::new();
    for entry in entries {
        let file_name_id = get_file_id(transaction, names, &entry.name);

        match insert_file(&transaction, &entry, file_name_id.unwrap()) {
            Ok(_) => {
                names.push((entry.name.clone(), 0));
            }
            Err(e) => {
                eprintln!("Failed to insert {}: {}", entry.name, e);
                return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
            }
        }

        if entry.is_directory {
            directories.push(entry.clone());
        }
    }

    Ok(directories)
}

fn get_file_id(
    transaction: &rusqlite::Transaction,
    names: &mut Vec<(String, i64)>,
    file_name: &str,
) -> io::Result<i64> {
    let file_name_id = names
        .iter()
        .find(|(name, _)| name == file_name)
        .map(|(_, id)| *id);

    if let Some(id) = file_name_id {
        return Ok(id);
    }

    let inserted_id = match insert_file_name(&transaction, &file_name) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("Failed to insert file name '{}': {}", &file_name, e);
            return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
        }
    };
    names.push((file_name.to_string(), inserted_id));
    Ok(inserted_id)
}
