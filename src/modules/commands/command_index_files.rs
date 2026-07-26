use std::io;

use indicatif::ProgressBar;

use crate::{
    modules::{
        file_entry::_types::FileEntry,
        logging,
        progress,
        search_files::try_get_dir_entries::try_get_dir_entries,
        sql::database::{get_connection, get_ignore_list, init_db, insert_file, insert_file_name, insert_skipped_path},
    },
    states::app_state,
};

struct InsertResult {
    directories: Vec<FileEntry>,
    file_count: usize,
    folder_count: usize,
    skipped: Option<(String, String)>,
}

pub fn index_directory(db_path: &str, root_dir: &str) -> io::Result<()> {
    {
        let mut conn = get_connection(db_path).map_err(|e| {
            logging::error(&format!("Failed to connect to database: {}", e));
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
        let tx = conn.transaction().map_err(|e| {
            logging::error(&format!("Failed to start transaction: {}", e));
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
        init_db(&tx).map_err(|e| {
            logging::error(&format!("Failed to initialize database: {}", e));
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
        tx.commit().map_err(|e| {
            logging::error(&format!("Failed to commit transaction: {}", e));
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
    }

    let root = if root_dir.ends_with('/') {
        root_dir.to_string()
    } else {
        format!("{}/", root_dir)
    };

    let ignore_list: Vec<String> = match get_connection(db_path) {
        Ok(conn) => get_ignore_list(&conn),
        Err(e) => {
            logging::warn(&format!("Could not load ignore list: {}", e));
            Vec::new()
        }
    };

    let mut names: Vec<(String, i64)> = vec![];
    let mut paths: Vec<String> = vec![root];
    let mut total_entries = 0usize;
    loop {
        let mut new_paths: Vec<String> = Vec::new();
        for path in &paths {
            let result = {
                let mut conn = get_connection(db_path).map_err(|e| {
                    logging::error(&format!("Failed to connect to database: {}", e));
                    io::Error::new(io::ErrorKind::Other, e.to_string())
                })?;
                let transaction = conn.transaction().map_err(|e| {
                    logging::error(&format!("Failed to start transaction: {}", e));
                    io::Error::new(io::ErrorKind::Other, e.to_string())
                })?;

                let result = get_and_insert_entries(&transaction, &mut names, path);

                match &result {
                    Ok(r) => {
                        if let Some((p, err)) = &r.skipped {
                            let _ = insert_skipped_path(&transaction, p, err);
                        }
                    }
                    Err(e) => {
                        let _ = insert_skipped_path(&transaction, path, &e.to_string());
                    }
                }

                transaction.commit().map_err(|e| {
                    logging::error(&format!("Failed to commit transaction: {}", e));
                    io::Error::new(io::ErrorKind::Other, e.to_string())
                })?;

                result
            };

            match result {
                Ok(r) => {
                    logging::info(&format!(
                        "Indexed '{}' — {} files, {} folders",
                        path.trim_end_matches('/'),
                        r.file_count,
                        r.folder_count
                    ));
                    total_entries += r.file_count + r.folder_count;
                    for d in &r.directories {
                        if ignore_list.iter().any(|ignored| ignored == &d.name) {
                            logging::debug(&format!("Ignoring folder '{}'", d.name));
                            continue;
                        }
                        new_paths.push(format!("{}/{}/", path.trim_end_matches('/'), d.name));
                    }
                }
                Err(e) => {
                    logging::warn(&format!("Skipping unreadable directory '{}': {}", path, e));
                }
            }
        }

        progress::update_dir(paths.first().unwrap_or(&String::new()), total_entries);

        if new_paths.is_empty() {
            break;
        }

        paths.clear();
        paths.extend(new_paths);
    }
    Ok(())
}

pub fn command_index_files(_pb: &ProgressBar) -> io::Result<bool> {
    logging::info("Starting directory listing...");

    let cwd = app_state::get_cwd();
    let db_path = "file_index.db";

    index_directory(db_path, &cwd)?;

    logging::info("Indexing complete.");
    progress::finish();
    Ok(false)
}

fn get_and_insert_entries(
    transaction: &rusqlite::Transaction,
    names: &mut Vec<(String, i64)>,
    path: &String,
) -> io::Result<InsertResult> {
    let entries = match try_get_dir_entries(&path, None) {
        Ok(entries) => entries,
        Err(e) => {
            return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
        }
    };

    let mut directories: Vec<FileEntry> = Vec::new();
    let mut file_count = 0usize;
    let mut folder_count = 0usize;

    for entry in &entries {
        let file_name_id = get_file_id(transaction, names, &entry.name);

        match insert_file(&transaction, &entry, file_name_id.unwrap()) {
            Ok(_) => {
                if entry.is_directory {
                    folder_count += 1;
                } else {
                    file_count += 1;
                }
            }
            Err(e) => {
                logging::warn(&format!(
                    "Skipping entry '{}' in '{}': {}",
                    entry.name, path, e
                ));
            }
        }

        if entry.is_directory {
            directories.push(entry.clone());
        }
    }

    Ok(InsertResult {
        directories,
        file_count,
        folder_count,
        skipped: None,
    })
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
            return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
        }
    };
    names.push((file_name.to_string(), inserted_id));
    Ok(inserted_id)
}
