use std::io;

use indicatif::ProgressBar;

use crate::{
    modules::file_entry::_types::FileEntry, modules::services::search_service::search_file_by_name,
};

use inquire::{Confirm, Select, Text};

#[derive(Debug, Clone, Copy)]
pub enum TargetKind {
    Files,
    Folders,
    Both,
}

#[derive(Debug, Clone, Copy)]
pub enum PatternKind {
    Exact,
    StartsWith,
    EndsWith,
    Contains,
}

#[derive(Debug, Clone, Copy)]
pub enum OrderKind {
    Asc,
    Desc,
}

fn request_user_input() -> (String, TargetKind, PatternKind, OrderKind, bool) {
    // File name input
    let file_name = Text::new("Enter the file name to search for:")
        .with_initial_value("file-a")
        .prompt()
        .unwrap();

    // Target type (file/folder/both)
    let target_kind_str = Select::new(
        "What do you want to search?",
        vec!["Files only", "Folders only", "Both"],
    )
    .with_starting_cursor(2)
    .prompt()
    .unwrap();

    let target_kind = match target_kind_str {
        "Files only" => TargetKind::Files,
        "Folders only" => TargetKind::Folders,
        "Both" => TargetKind::Both,
        _ => unreachable!(),
    };

    // Pattern matching
    let pattern_kind = Select::new(
        "Choose a pattern matching mode:",
        vec!["Exact", "Starts with", "Ends with", "Contains"],
    )
    .with_starting_cursor(0)
    .prompt()
    .unwrap();

    let pattern_kind = match pattern_kind {
        "Exact" => PatternKind::Exact,
        "Starts with" => PatternKind::StartsWith,
        "Ends with" => PatternKind::EndsWith,
        "Contains" => PatternKind::Contains,
        _ => unreachable!(),
    };

    // Sort order
    let order = Select::new("Choose the sort order:", vec!["Ascending", "Descending"])
        .with_starting_cursor(0)
        .prompt()
        .unwrap();

    let order_kind = match order {
        "Ascending" => OrderKind::Asc,
        "Descending" => OrderKind::Desc,
        _ => unreachable!(),
    };

    // Verbose logging
    let verbose = Confirm::new("Enable verbose logging?")
        .with_default(false)
        .prompt()
        .unwrap();

    (file_name, target_kind, pattern_kind, order_kind, verbose)
}

pub fn command_search_file(pb: &ProgressBar) -> io::Result<bool> {
    pb.finish();

    let (file_name, target_kind, pattern_kind, order_kind, verbose) = request_user_input();

    pb.println(format!(
        "[info] Searching for file: '{}'\n[info] Pattern: {:?}, Order: {:?}, Verbose: {}",
        file_name, pattern_kind, order_kind, verbose
    ));

    let entries =
        search_file_by_name(&file_name, target_kind, pattern_kind, order_kind).map_err(|e| {
            eprintln!("Error searching for file: {}", e);
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;

    // group entries by hash
    let mut entries_by_hash: std::collections::HashMap<String, Vec<FileEntry>> =
        std::collections::HashMap::new();
    for entry in entries {
        entries_by_hash
            .entry(
                entry
                    .hash
                    .clone()
                    .unwrap_or_else(|| "<no hash>".to_string()),
            )
            .or_insert_with(Vec::new)
            .push(entry);
    }

    // Print grouped entries
    for (hash, entries) in &entries_by_hash {
        println!("[info] Hash: {}", hash);

        for entry in entries {
            println!(
                "Entry: {}, is_directory: {}, is_file: {}, is_symlink: {}, modified: {}, hash: {}",
                entry.name,
                entry.is_directory,
                entry.is_file,
                entry.is_symlink,
                entry.modified.unwrap_or(0),
                entry
                    .hash
                    .clone()
                    .unwrap_or_else(|| "<no hash>".to_string())
            );
        }
    }

    // for entry in entries {
    //     println!(
    //         "Entry: {}, is_directory: {}, is_file: {}, is_symlink: {}, modified: {}",
    //         entry.name, entry.is_directory, entry.is_file, entry.is_symlink, entry.modified.unwrap_or(0)
    //     );
    // }

    Ok(false)
}
