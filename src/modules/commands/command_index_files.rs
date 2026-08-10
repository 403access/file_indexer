use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use indicatif::ProgressBar;

use crate::{
    modules::{
        file_entry::_types::FileEntry,
        logging,
        processes,
        progress,
        search_files::try_get_dir_entries::try_get_dir_entries,
        sql::database::{get_connection, get_ignore_rules, get_child_directories, get_or_insert_file_name, init_db, insert_file, insert_file_name, insert_skipped_path, is_directory_indexed, mark_directory_traversed, update_duplicate_hashes_incremental, IgnoreRule},
    },
    states::app_state,
};

struct InsertResult {
    directories: Vec<FileEntry>,
    file_count: usize,
    folder_count: usize,
    skipped: Option<(String, String)>,
    inserted_hashes: Vec<String>,
}

/// Index `root_dir` into the database.
///
/// - `pause_flag`: shared web-request pause refcount (yield while > 0)
/// - `process_id`: optional process registry id for UI pause/stop controls
pub fn index_directory(
    db_path: &str,
    root_dir: &str,
    pause_flag: Option<Arc<AtomicUsize>>,
    process_id: Option<u64>,
) -> io::Result<()> {
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

    let root_trimmed = root.trim_end_matches('/').to_string();

    // Insert root directory as a row (parent_path = NULL, traversed = 0)
    {
        let mut conn = get_connection(db_path).map_err(|e| {
            logging::error(&format!("Failed to connect to database: {}", e));
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
        let tx = conn.transaction().map_err(|e| {
            logging::error(&format!("Failed to start transaction: {}", e));
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
        let root_entry = FileEntry {
            path: Some(root_trimmed.clone()),
            name: root_trimmed
                .rsplit('/')
                .next()
                .unwrap_or(&root_trimmed)
                .to_string(),
            size: 0,
            created: None,
            modified: None,
            accessed: None,
            hash: None,
            is_directory: true,
            is_file: false,
            is_symlink: false,
            parent_path: None,
        };
        let name_id = get_or_insert_file_name(&tx, &root_entry.name).map_err(|e| {
            logging::error(&format!("Failed to insert root name: {}", e));
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
        let _ = insert_file(&tx, &root_entry, name_id, None).map_err(|e| {
            logging::error(&format!("Failed to insert root: {}", e));
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
        tx.commit().map_err(|e| {
            logging::error(&format!("Failed to commit transaction: {}", e));
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
    }

    let ignore_rules: Vec<IgnoreRule> = match get_connection(db_path) {
        Ok(conn) => get_ignore_rules(&conn),
        Err(e) => {
            logging::warn(&format!("Could not load ignore rules: {}", e));
            Vec::new()
        }
    };

    let mut names: Vec<(String, i64)> = vec![];
    let mut paths: Vec<String> = vec![root];
    let mut total_entries = 0usize;
    loop {
        let mut new_paths: Vec<String> = Vec::new();
        for path in &paths {
            // Honor pause/stop from web traffic and the processes UI
            wait_if_paused(pause_flag.as_ref(), process_id)?;

            // Check if this directory was already traversed
            let already_traversed = {
                let conn = get_connection(db_path).map_err(|e| {
                    logging::error(&format!("Failed to connect to database: {}", e));
                    io::Error::new(io::ErrorKind::Other, e.to_string())
                })?;
                is_directory_indexed(&conn, path)
            };

            if already_traversed {
                // Directory already processed — queue its children from DB
                let conn = get_connection(db_path).map_err(|e| {
                    logging::error(&format!("Failed to connect to database: {}", e));
                    io::Error::new(io::ErrorKind::Other, e.to_string())
                })?;
                let children = get_child_directories(&conn, path.trim_end_matches('/')).unwrap_or_default();
                for child in children {
                    let child_with_slash = format!("{}/", child);
                    let child_name = child_with_slash.trim_start_matches(path.trim_end_matches('/')).trim_start_matches('/').trim_end_matches('/');
                    let parent_path = std::path::Path::new(path.trim_end_matches('/'));
                    let should_skip = ignore_rules.iter().any(|rule| {
                        rule.name == child_name && rule.should_skip(parent_path)
                    });
                    if !should_skip {
                        new_paths.push(child_with_slash);
                    }
                }
                logging::debug(&format!("Resume: skipping already-traversed '{}'", path.trim_end_matches('/')));
                continue;
            }

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

                // Incrementally update duplicate hashes for newly inserted files
                if let Ok(ref r) = result {
                    if !r.inserted_hashes.is_empty() {
                        update_duplicate_hashes_incremental(&conn, &r.inserted_hashes);
                    }
                }

                result
            };

            match result {
                Ok(r) => {
                    // Mark this directory as traversed
                    {
                        let mut conn = get_connection(db_path).map_err(|e| {
                            logging::error(&format!("Failed to connect to database: {}", e));
                            io::Error::new(io::ErrorKind::Other, e.to_string())
                        })?;
                        let tx = conn.transaction().map_err(|e| {
                            logging::error(&format!("Failed to start transaction: {}", e));
                            io::Error::new(io::ErrorKind::Other, e.to_string())
                        })?;
                        let _ = mark_directory_traversed(&tx, path.trim_end_matches('/'));
                        tx.commit().map_err(|e| {
                            logging::error(&format!("Failed to commit transaction: {}", e));
                            io::Error::new(io::ErrorKind::Other, e.to_string())
                        })?;
                    }

                    logging::info(&format!(
                        "Indexed '{}' — {} files, {} folders",
                        path.trim_end_matches('/'),
                        r.file_count,
                        r.folder_count
                    ));
                    total_entries += r.file_count + r.folder_count;
                    for d in &r.directories {
                        let parent_path = std::path::Path::new(path.trim_end_matches('/'));
                        if let Some(rule) = ignore_rules
                            .iter()
                            .find(|rule| rule.name == d.name && rule.should_skip(parent_path))
                        {
                            logging::debug(&format!(
                                "Ignored folder '{}' via rule '{}'",
                                d.name,
                                rule.to_raw()
                            ));
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

    index_directory(db_path, &cwd, None, None)?;

    logging::info("Indexing complete.");
    progress::finish();
    Ok(false)
}

/// Wait while web requests or the processes UI have this job paused.
/// Returns `Interrupted` if the process was stopped.
fn wait_if_paused(
    pause_flag: Option<&Arc<AtomicUsize>>,
    process_id: Option<u64>,
) -> io::Result<()> {
    loop {
        if let Some(pid) = process_id {
            if processes::is_stopped(pid) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Indexing stopped by user",
                ));
            }
        }

        let web_paused = pause_flag
            .map(|f| f.load(Ordering::SeqCst) > 0)
            .unwrap_or(false);
        let proc_paused = process_id.map(processes::is_paused).unwrap_or(false);

        if !web_paused && !proc_paused {
            return Ok(());
        }

        std::thread::sleep(std::time::Duration::from_millis(50));
    }
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

    let parent_path = Some(path.trim_end_matches('/'));
    let mut directories: Vec<FileEntry> = Vec::new();
    let mut file_count = 0usize;
    let mut folder_count = 0usize;
    let mut inserted_hashes: Vec<String> = Vec::new();

    for entry in &entries {
        let file_name_id = get_file_id(transaction, names, &entry.name);

        match insert_file(&transaction, &entry, file_name_id.unwrap(), parent_path) {
            Ok(id) => {
                if id > 0 {
                    // New file inserted (not a duplicate key)
                    if let Some(hash) = &entry.hash {
                        if !hash.is_empty() {
                            inserted_hashes.push(hash.clone());
                        }
                    }
                }
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
        inserted_hashes,
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
        Err(_) => {
            // Name already exists — query the existing id
            transaction
                .query_row(
                    "SELECT id FROM file_names WHERE name = ?1",
                    [file_name],
                    |row| row.get(0),
                )
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?
        }
    };
    names.push((file_name.to_string(), inserted_id));
    Ok(inserted_id)
}
