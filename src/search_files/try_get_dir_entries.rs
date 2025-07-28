use std::{fs, io};

use file_entry::_types::FileEntry;
use file_entry::convert::convert_from_dir;

use file_entry::sort::sort_dir_entries;
use file_entry::sort::SortOrder;

use index_files::_command::check_input;

use crate::file_entry;
use crate::index_files;

/**
 * Tries to get directory entries from the specified path.
 * Optionally sorts the entries based on the provided sort order.
 * Converts each directory entry to a `FileEntry` type.
 */
pub fn try_get_dir_entries(
    path: &str,
    sort_order: Option<SortOrder>,
) -> io::Result<Vec<FileEntry>> {
    println!("Listing files in directory: {}", path);
    if let Err(e) = check_input(path) {
        return Err(e);
    }
    println!("Input path is valid: {}", path);

    // let entries = match fs::read_dir(path) {
    //     Ok(entries) => entries,
    //     Err(e) => {
    //         eprintln!("Error reading directory: {}", e);
    //         return Err(e);
    //     }
    // };

    // let read_dir_result = fs::read_dir(path);
    // if read_dir_result.is_err() {
    //     eprintln!("Error reading directory: {}", read_dir_result.as_ref().err().unwrap());
    //     return Err(io::Error::new(io::ErrorKind::Other, "Failed to read directory"));
    // }
    // let entries = read_dir_result.unwrap();

    // get entries and use map error handling
    let entries = fs::read_dir(path).map_err(|e| {
        eprintln!("Error reading directory: {}", e);
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
                        eprintln!("Error converting entry: {}", e);
                        return Err(io::Error::new(
                            io::ErrorKind::Other,
                            "Failed to convert directory entry",
                        ));
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading directory entry: {}", e);
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Failed to read directory entry",
                ));
            }
        }
    }

    let count = dir_entries.len();
    println!("Total entries: {}", count);

    // Set default sort order if not provided
    let default_sort_order = SortOrder::AaBbCc;
    let sort_order = sort_order.unwrap_or(default_sort_order);
    sort_dir_entries(sort_order, &mut dir_entries);

    Ok(dir_entries)
}
