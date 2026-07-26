use std::{fs, io};

use file_entry::_types::FileEntry;
use file_entry::convert::convert_from_dir;

use file_entry::sort::sort_dir_entries;
use file_entry::sort::SortOrder;

use index_files::_command::check_input;

use crate::modules::file_entry;
use crate::modules::index_files;
use crate::modules::logging;

pub fn try_get_dir_entries(
    path: &str,
    sort_order: Option<SortOrder>,
) -> io::Result<Vec<FileEntry>> {
    if let Err(e) = check_input(path) {
        return Err(e);
    }

    let entries = fs::read_dir(path).map_err(|e| {
        logging::error(&format!("Error reading directory: {}", e));
        io::Error::new(io::ErrorKind::Other, "Failed to read directory")
    })?;

    let mut dir_entries: Vec<FileEntry> = Vec::new();

    for entry in entries {
        match entry {
            Ok(dir_entry) => {
                let file_entry = convert_from_dir(dir_entry);
                match file_entry {
                    Ok(entry) => dir_entries.push(entry),
                    Err(e) => {
                        logging::warn(&format!(
                            "Skipping unreadable entry in '{}': {}",
                            path, e
                        ));
                    }
                }
            }
            Err(e) => {
                logging::warn(&format!(
                    "Skipping unreadable entry in '{}': {}",
                    path, e
                ));
            }
        }
    }

    // Set default sort order if not provided
    let default_sort_order = SortOrder::AaBbCc;
    let sort_order = sort_order.unwrap_or(default_sort_order);
    sort_dir_entries(sort_order, &mut dir_entries);

    Ok(dir_entries)
}
