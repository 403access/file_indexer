use std::fs::{self, FileType};
use std::io::Read;
use std::path::Path;

use std::io::Seek;

use crate::modules::file_entry::_types::FileEntry;
use crate::modules::logging;

pub struct FileFlags {
    pub is_directory: bool,
    pub is_file: bool,
    pub is_symlink: bool,
}

pub fn get_file_flags(file_type: FileType) -> FileFlags {
    FileFlags {
        is_directory: file_type.is_dir(),
        is_file: file_type.is_file(),
        is_symlink: file_type.is_symlink(),
    }
}

/**
 * Convert fs::DirEntry to FileEntry
 */
pub fn convert_from_dir(entry: fs::DirEntry) -> Result<FileEntry, std::io::Error> {
    let path = entry.path();
    let hash = if path.is_file() {
        // let computed_hash = compute_sha256(&path).map_err(|e| {
        //     eprintln!("Error computing hash for file {}: {}", path.display(), e);
        //     e
        // })?;
        // Some(computed_hash)
        // Some(compute_sha256(&path))
        // Some(String::new())
        // None
        Some(compute_sha256_fast(&path).map_err(|e| {
            logging::error(&format!("Error computing hash for file {}: {}", path.display(), e));
            e
        })?)
    } else {
        // Some(String::new())
        None
    };

    let file_type = match entry.file_type() {
        Ok(ft) => ft,
        Err(e) => {
            logging::error(&format!("Error getting file type: {}", e));
            return Err(e);
        }
    };
    let flags = get_file_flags(file_type);

    // Get metadata for the entry or return an error if it fails
    let metadata = match entry.metadata() {
        Ok(m) => m,
        Err(e) => {
            logging::error(&format!("Error getting metadata for entry: {}", e));
            return Err(e);
        }
    };
    // let metadata = entry.metadata().map_err(|e| {
    //     eprintln!("Error getting metadata for entry: {}", e);
    //     e
    // })?;

    let created = system_time_to_unix_secs(metadata.created());
    let modified = system_time_to_unix_secs(metadata.modified());
    let accessed = system_time_to_unix_secs(metadata.accessed());

    Ok(FileEntry {
        // path: None,
        path: Some(path.to_string_lossy().into_owned()),
        is_directory: flags.is_directory,
        is_file: flags.is_file,
        is_symlink: flags.is_symlink,
        created,
        modified,
        accessed,
        name: entry.file_name().to_string_lossy().into_owned(),
        size: entry.metadata().map_or(0, |m| m.len()),
        hash,
        parent_path: None,
    })
}

pub fn convert_from_row(row: &rusqlite::Row) -> rusqlite::Result<FileEntry> {
    Ok(FileEntry {
        is_directory: row.get::<_, i32>("is_directory")? != 0,
        is_file: row.get::<_, i32>("is_file")? != 0,
        is_symlink: row.get::<_, i32>("is_symlink")? != 0,
        path: row.get("path")?,
        name: row.get("name")?,
        size: row.get("size")?,
        created: None,
        modified: row.get::<_, Option<f64>>("modified")?.map(|ts| ts as u64),
        accessed: None,
        hash: row.get("hash")?,
        parent_path: row.get("parent_path")?,
    })
}

pub fn convert_from_rows(rows: &mut rusqlite::Rows<'_>) -> rusqlite::Result<Vec<FileEntry>> {
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        results.push(convert_from_row(row)?);
    }
    Ok(results)
}

// /// Compute SHA-256 hash of a file at the given path
// fn compute_sha256(path: &Path) -> Result<String, std::io::Error> {
//     println!("Computing SHA-256 hash for file: {}", path.display());

//     use sha2::{Digest, Sha256};
//     let mut file = match fs::File::open(path) {
//         Ok(f) => f,
//         Err(_) => {
//             return Err(std::io::Error::new(
//                 std::io::ErrorKind::NotFound,
//                 "File not found",
//             ))
//         }
//     };
//     let mut hasher = Sha256::new();
//     let mut buffer = [0u8; 4096];
//     loop {
//         match file.read(&mut buffer) {
//             Ok(0) => break,
//             Ok(n) => {
//                 hasher.update(&buffer[..n]);
//             }
//             Err(_) => {
//                 return Err(std::io::Error::new(
//                     std::io::ErrorKind::Other,
//                     "Failed to read file",
//                 ))
//             }
//         }
//     }
//     println!("SHA-256 hash computed successfully.");
//     let hash = format!("{:x}", hasher.finalize());
//     println!("SHA-256 hash: {}", hash);
//     Ok(hash)
// }

fn compute_sha256_fast(path: &Path) -> Result<String, std::io::Error> {
    use sha2::{Digest, Sha256};
    let mut file = fs::File::open(path)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "File not found"))?;
    let mut hasher = Sha256::new();

    // Generate hash by hashing each of the following:
    // 1. File size
    // 2. First 4096 bytes of the file
    // 3. Mid section of the file (if larger than 8192 bytes)
    // 4. Last 4096 bytes of the file (if larger than 4096 bytes)
    let file_size = file
        .metadata()
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Failed to get file metadata"))?
        .len();
    hasher.update(file_size.to_le_bytes());

    // if file is empty, return the hash of 0
    if file_size != 0 {
        let mut buffer = [0u8; 4096];
        // Read the first 4096 bytes
        match file.read(&mut buffer) {
            Ok(n) if n > 0 => hasher.update(&buffer[..n]),
            Ok(_) => {}
            Err(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to read file",
                ))
            }
        }

        // If the file is larger than 4096 bytes, read the last 4096 bytes
        if file_size > 4096 {
            file.seek(std::io::SeekFrom::End(-4096)).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::Other, "Failed to seek in file")
            })?;
            match file.read(&mut buffer) {
                Ok(n) if n > 0 => hasher.update(&buffer[..n]),
                Ok(_) => {}
                Err(_) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Failed to read file",
                    ))
                }
            }
        }
    }

    // print hash then return it
    let hash = format!("{:x}", hasher.finalize());
    Ok(hash)
}

fn system_time_to_unix_secs(time: std::io::Result<std::time::SystemTime>) -> Option<u64> {
    time.ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

/// Sub-second Unix timestamp (fractional seconds). Used as the reconcile key:
/// a directory whose stored mtime matches its disk mtime needs no re-read.
pub fn system_time_to_unix_f64(time: std::io::Result<std::time::SystemTime>) -> Option<f64> {
    time.ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64())
}
