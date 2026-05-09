use std::env::temp_dir;
use std::fs::{self, create_dir_all, remove_dir, DirBuilder, File};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const TEST_DATABASE_FILE_PATH: &str = "test";

use crate::modules::commands::command_init_db::_init_db;

pub fn database_exists() -> bool {
    return fs::exists(TEST_DATABASE_FILE_PATH).unwrap();
}

pub fn create_database() -> PathBuf {
    let database_file_path = Path::new("file_index.db");
    let database_file_path_buf = database_file_path.to_path_buf();

    let result = _init_db(&database_file_path_buf);
    assert!(result.is_ok());

    return database_file_path_buf;
}

pub fn delete_database() {
    let result = fs::remove_file("file_index.db");
    assert!(result.is_ok());
}

///
/// Creates temp folder and returns its path.
///
pub fn create_temp_folder() -> PathBuf {
    let temp_dir_path = temp_dir();
    println!("Temp dir path: {}", temp_dir_path.display());

    let id = Uuid::new_v4();
    println!("Generated UUID: {}", id);

    let path = temp_dir_path.join(id.to_string());
    println!("Individual dir path: {}", path.display());

    // DirBuilder::new()
    //     .recursive(true)
    //     .create(path.clone())
    //     .unwrap();
    create_dir_all(&path);

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

pub fn create_test_data() -> PathBuf {
    // |-test_data
    let temp_folder_path = create_temp_folder();
    // |  |-file-a
    create_file(&temp_folder_path, "file-a");
    // |  |-folder-c
    let folder_c_path = temp_folder_path.join("folder-c");
    DirBuilder::new().create(folder_c_path.clone()).unwrap();
    // |  |  |-file-c-1
    create_file(&folder_c_path, "file-c-1");
    // |  |  |-folder-d
    let folder_c_folder_d_path = folder_c_path.join("folder-d");
    // |  |  |  |-folder-b
    let folder_c_folder_d_folder_b_path = folder_c_folder_d_path.join("folder-b");
    create_file(&folder_c_folder_d_folder_b_path, "file-b-1");
    // |  |  |  |  |-file-b-1
    // |  |  |  |-folder-a
    // |  |  |  |  |-file-a-2
    // |  |  |  |  |-file-a-1
    // |  |  |-file-c-2
    // |  |  |-file-a-1
    // |  |  |-file-c-3
    // |  |-folder-b
    // |  |  |-file-b-1
    // |  |-folder-e
    // |  |  |-folder-c
    // |  |  |  |-file-c-1
    // |  |  |  |-file-c-2
    // |  |  |-folder-b
    // |  |  |  |-file-b-1
    // |  |  |-folder-a
    // |  |  |  |-file-a-2
    // |  |  |  |-file-a-1
    // |  |-file-b
    // |  |-folder-f
    // |  |  |-file-f
    // |  |  |-file-a
    // |  |  |-file-b
    // |  |-folder-a
    // |  |  |-file-a-2
    // |  |  |-file-a-1

    return temp_folder_path;
}

pub fn delete_test_data(temp_folder_path_buf: &PathBuf) {
    remove_dir(temp_folder_path_buf);
}
