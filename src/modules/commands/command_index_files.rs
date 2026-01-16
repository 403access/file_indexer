use std::io;

use indicatif::ProgressBar;

use crate::{
    modules::file_entry::_types::FileEntry,
    modules::search_files::try_get_dir_entries::try_get_dir_entries,
    modules::sql::database::{get_connection, insert_file, insert_file_name},
};

const STARTING_PATH: &str =
    "/Users/olivermolnar/Desktop/Projects/file_indexer/tests/data/sample-directory/";

pub fn command_index_files(_pb: &ProgressBar) -> io::Result<bool> {
    println!("Starting directory listing...");

    let mut names: Vec<(String, i64)> = vec![];

    let mut conn = get_connection("file_index.db").map_err(|e| {
        eprintln!("Failed to connect to database: {}", e);
        io::Error::new(io::ErrorKind::Other, e.to_string())
    })?;
    println!("Database connection established.");

    let mut paths: Vec<String> = vec![STARTING_PATH.to_string()];
    println!("Starting to process directories...");
    loop {
        println!("Paths to iterate through:");
        for path in &paths {
            println!("- {}", path);
        }

        let transaction = match conn.transaction() {
            Ok(tx) => tx,
            Err(e) => {
                eprintln!("Failed to start transaction: {}", e);
                return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
            }
        };

        // let mut directories = Vec::new();
        let mut new_paths: Vec<String> = Vec::new();
        for path in &paths {
            match get_and_insert_entries(&transaction, &mut names, path) {
                Ok(dirs) => {
                    // directories.extend(dirs);
                    new_paths.extend(
                        dirs.iter()
                            .filter(|d| d.is_directory)
                            .map(|d| format!("{}/{}/", path, d.name)),
                    );
                }
                Err(e) => {
                    eprintln!("Failed to get entries for path '{}': {}", path, e);
                    return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
                }
            }
        }

        transaction.commit().map_err(|e| {
            eprintln!("Failed to commit transaction: {}", e);
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
        println!("Transaction committed successfully.");

        if new_paths.is_empty() {
            println!("No more directories to process.");
            break;
        }

        // Update paths for the next iteration
        paths.clear();
        paths.extend(new_paths);
    }
    Ok(false)
}

fn get_and_insert_entries(
    transaction: &rusqlite::Transaction,
    names: &mut Vec<(String, i64)>,
    path: &String,
) -> io::Result<Vec<FileEntry>> {
    let entries = match try_get_dir_entries(&path, None) {
        Ok(entries) => {
            println!("Successfully retrieved directory entries.");
            println!("Found {} entries.", entries.len());
            entries
        }
        Err(e) => {
            eprintln!("Failed to retrieve directory entries: {}", e);
            return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
        }
    };

    let mut directories: Vec<FileEntry> = Vec::new();
    let mut inserted_files = 0;
    for entry in entries {
        println!("Processing entry: {}", entry.name);
        // get id of the file name from the names vector
        let file_name_id = get_file_id(transaction, names, &entry.name);

        match insert_file(&transaction, &entry, file_name_id.unwrap()) {
            Ok(inserted_rowid) => {
                println!("Insert file '{}' with id '{}'", entry.name, inserted_rowid);
                inserted_files += 1;
                names.push((entry.name.clone(), inserted_rowid));
            }
            Err(e) => {
                eprintln!("Failed to insert {}: {}", entry.name, e);
                return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
            }
        }

        if entry.is_directory {
            println!("This is a directory.");
            directories.push(entry.clone());
        } else if entry.is_file {
            println!("This is a file.");
        } else if entry.is_symlink {
            println!("This is a symlink.");
        } else {
            println!("Unknown type.");
        }
    }

    println!("Inserted {} entries into the database.", inserted_files);

    return Ok(directories);
}

fn get_file_id(
    transaction: &rusqlite::Transaction,
    names: &mut Vec<(String, i64)>,
    file_name: &str,
) -> io::Result<i64> {
    // get id of the file name from the names vector
    let file_name_id = names
        .iter()
        .find(|(name, _)| name == file_name)
        .map(|(_, id)| *id);
    // If the file name ID is already in the names vector, skip insertion
    if let Some(id) = file_name_id {
        println!(
            "File name '{}' already exists with ID '{}', skipping insertion.",
            &file_name, id
        );
        return Ok(id);
    }

    // Insert the new entry name into the names vector
    let inserted_id = match insert_file_name(&transaction, &file_name) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("Failed to insert file name '{}': {}", &file_name, e);
            // continue; // Skip this entry if insertion fails
            return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
        }
    };
    names.push((file_name.to_string(), inserted_id));
    return Ok(inserted_id);
}
