use rusqlite::Transaction;

use crate::{
    modules::file_entry::{_types::FileEntry, convert::convert_from_rows},
    modules::sql::database::reset_duplicates_table,
};

pub fn get_duplicates(tx: &Transaction, limit: Option<u64>) -> rusqlite::Result<Vec<FileEntry>> {
    reset_duplicates_table(tx)?;

    let limit = limit.unwrap_or(100);
    let sql = format!(
        "
        SELECT f.path, fn.name, f.size, f.modified, f.hash,
               f.is_directory, f.is_file, f.is_symlink, f.parent_path
        FROM files f
        JOIN file_names fn ON f.file_name_id = fn.id
        JOIN duplicate_hashes d ON f.hash = d.hash
        LIMIT {}
        ",
        limit
    );
    let mut stmt = tx.prepare(&sql)?;

    println!("[info] Getting duplicates...");
    let mut rows = stmt.query(rusqlite::params![])?;

    convert_from_rows(&mut rows)
}

// Next we need to know not only the duplicate files as we already implemented in the `get_duplicates` function,
// but also determine similarities, meaning amount of same duplicate files, within directories.

// Get files sorted by occurrences ASC
// Get first file
// Based on that file we get all folders (DESC by files amount)
//    - for this maybe we should have an additional column for files count or a separate table
// that contain a file with that hash - even if name is different
// We store and remember all relevant folders
// We store in a record / (literally hash)map which folders contain that file.
// Now, we take the first folder and retrieve all files
// go through all files and repeat all steps from the step of noting which folders contains that file
// Once all files of that current focused folder is checked we continue with the next folder from the step in which we retrieved the folders
//
// The idea: We start with folders that contain less of same files to not bloat the memory.
//
// The remaining question is how do we store that map in our database?
// Is it an n-to-m map?
// Also, what is the actual target? I guess it is about to find folders that are truely the same
// or how many percentage the same / amount of files same
// Also, should we change the strategy to check files from the most nested folder and to always one parent up?
// Okay, so we might need to update the index function by storing one more column "parent" that contains
// the index of the parent folder
// With that, we can start from the folder that has the highest entry (id) for the parent folder.
// Else we would need to do something like count all slashes (/) in the path for each folder to get the most nested folder
