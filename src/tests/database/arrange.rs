use std::env::temp_dir;
use std::fs::{self, create_dir_all, remove_dir_all, File};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const TEST_DATABASE_FILE_PATH: &str = "test";

use crate::modules::commands::command_init_db::_init_db;

pub fn database_exists() -> bool {
    return fs::exists(TEST_DATABASE_FILE_PATH).unwrap();
}

pub fn create_database(database_file_path_buf: &PathBuf) -> PathBuf {
    let database_file_path = Path::new(database_file_path_buf);
    let database_file_path_buf = database_file_path.to_path_buf();

    let result = _init_db(&database_file_path_buf);
    assert!(result.is_ok());

    return database_file_path_buf;
}

pub fn delete_database(database_file_path_buf: &PathBuf) {
    let result = fs::remove_file(database_file_path_buf);
    assert!(result.is_ok());
}

///
/// Creates temp folder and returns its path.
///
pub fn create_temp_folder() -> PathBuf {
    let temp_dir_path = temp_dir();
    let id = Uuid::new_v4();
    let path = temp_dir_path.join(id.to_string());
    create_dir_all(&path).unwrap();
    assert!(fs::metadata(&path).unwrap().is_dir());
    return path;
}

///
/// Creates a file with a name and content equal to the provided name.
///
pub fn create_file(dir_path_buf: &PathBuf, name: &str) {
    let dir_path = Path::new(dir_path_buf);
    let mut file = File::create(dir_path.join(name)).unwrap();
    let name_bytes = name.as_bytes();
    let written_size = file.write(name_bytes).unwrap();
    assert!(name_bytes.len() == written_size)
}

/// Creates the full sample directory tree matching src/tests/data/sample-directory/.
///
/// ```text
/// <temp>/
/// ├── file-a
/// ├── file-b
/// ├── folder-a/
/// │   ├── file-a-1
/// │   └── file-a-2
/// ├── folder-b/
/// │   └── file-b-1
/// ├── folder-c/
/// │   ├── file-a-1
/// │   ├── file-c-1
/// │   ├── file-c-2
/// │   ├── file-c-3
/// │   └── folder-d/
/// │       ├── folder-a/
/// │       │   ├── file-a-1
/// │       │   └── file-a-2
/// │       └── folder-b/
/// │           └── file-b-1
/// ├── folder-e/
/// │   ├── folder-a/
/// │   │   ├── file-a-1
/// │   │   └── file-a-2
/// │   ├── folder-b/
/// │   │   └── file-b-1
/// │   └── folder-c/
/// │       ├── file-c-1
/// │       └── file-c-2
/// └── folder-f/
///     ├── file-a
///     ├── file-b
///     └── file-f
/// ```
pub fn create_test_data() -> PathBuf {
    let root = create_temp_folder();

    // Root files
    create_file(&root, "file-a");
    create_file(&root, "file-b");

    // folder-a/
    let folder_a = root.join("folder-a");
    create_dir_all(&folder_a).unwrap();
    create_file(&folder_a, "file-a-1");
    create_file(&folder_a, "file-a-2");

    // folder-b/
    let folder_b = root.join("folder-b");
    create_dir_all(&folder_b).unwrap();
    create_file(&folder_b, "file-b-1");

    // folder-c/
    let folder_c = root.join("folder-c");
    create_dir_all(&folder_c).unwrap();
    create_file(&folder_c, "file-a-1");
    create_file(&folder_c, "file-c-1");
    create_file(&folder_c, "file-c-2");
    create_file(&folder_c, "file-c-3");

    // folder-c/folder-d/
    let folder_c_d = folder_c.join("folder-d");
    create_dir_all(&folder_c_d).unwrap();

    // folder-c/folder-d/folder-a/
    let folder_c_d_a = folder_c_d.join("folder-a");
    create_dir_all(&folder_c_d_a).unwrap();
    create_file(&folder_c_d_a, "file-a-1");
    create_file(&folder_c_d_a, "file-a-2");

    // folder-c/folder-d/folder-b/
    let folder_c_d_b = folder_c_d.join("folder-b");
    create_dir_all(&folder_c_d_b).unwrap();
    create_file(&folder_c_d_b, "file-b-1");

    // folder-e/
    let folder_e = root.join("folder-e");
    create_dir_all(&folder_e).unwrap();

    // folder-e/folder-a/
    let folder_e_a = folder_e.join("folder-a");
    create_dir_all(&folder_e_a).unwrap();
    create_file(&folder_e_a, "file-a-1");
    create_file(&folder_e_a, "file-a-2");

    // folder-e/folder-b/
    let folder_e_b = folder_e.join("folder-b");
    create_dir_all(&folder_e_b).unwrap();
    create_file(&folder_e_b, "file-b-1");

    // folder-e/folder-c/
    let folder_e_c = folder_e.join("folder-c");
    create_dir_all(&folder_e_c).unwrap();
    create_file(&folder_e_c, "file-c-1");
    create_file(&folder_e_c, "file-c-2");

    // folder-f/
    let folder_f = root.join("folder-f");
    create_dir_all(&folder_f).unwrap();
    create_file(&folder_f, "file-a");
    create_file(&folder_f, "file-b");
    create_file(&folder_f, "file-f");

    return root;
}

pub fn delete_test_data(temp_folder_path_buf: &PathBuf) {
    remove_dir_all(temp_folder_path_buf).unwrap();
}
