use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::modules::commands::command_index_files::index_directory;
use crate::modules::sql::database::get_connection;
use crate::modules::sql::duplicate_folders_materialized::refresh_duplicate_folder_groups;
use crate::tests::database::arrange::{create_temp_folder, delete_test_data};

fn write_file(dir: &PathBuf, name: &str, content: &[u8]) {
    let mut file = fs::File::create(dir.join(name)).unwrap();
    file.write_all(content).unwrap();
}

fn index(temp: &PathBuf) -> rusqlite::Connection {
    let db_path = temp.join("file_index.db");
    index_directory(db_path.to_str().unwrap(), temp.to_str().unwrap(), None).unwrap();
    let conn = get_connection(db_path.to_str().unwrap()).unwrap();
    conn
}

#[test]
fn duplicate_folder_groups_table_is_created() {
    let temp = create_temp_folder();
    let conn = index(&temp);

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='duplicate_folder_groups'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);

    delete_test_data(&temp);
}

#[test]
fn refresh_duplicate_folder_groups_populates_groups() {
    let temp = create_temp_folder();

    let folder_a = temp.join("folder-a");
    let folder_b = temp.join("folder-b");
    fs::create_dir(&folder_a).unwrap();
    fs::create_dir(&folder_b).unwrap();

    let shared = b"shared content";
    write_file(&folder_a, "dup.txt", shared);
    write_file(&folder_b, "dup.txt", shared);

    let conn = index(&temp);
    refresh_duplicate_folder_groups(&conn);

    let group_count: i64 = conn
        .query_row("SELECT COUNT(DISTINCT group_id) FROM duplicate_folder_groups", [], |row| row.get(0))
        .unwrap();
    assert!(group_count > 0, "Expected at least one duplicate folder group, got {}", group_count);

    let total_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM duplicate_folder_groups", [], |row| row.get(0))
        .unwrap();
    assert_eq!(total_rows, 2, "Expected 2 folder rows, got {}", total_rows);

    delete_test_data(&temp);
}

#[test]
fn refresh_duplicate_folder_groups_is_idempotent() {
    let temp = create_temp_folder();

    let folder_a = temp.join("folder-a");
    let folder_b = temp.join("folder-b");
    fs::create_dir(&folder_a).unwrap();
    fs::create_dir(&folder_b).unwrap();

    let shared = b"idempotent content";
    write_file(&folder_a, "dup.txt", shared);
    write_file(&folder_b, "dup.txt", shared);

    let conn = index(&temp);

    refresh_duplicate_folder_groups(&conn);
    let first_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM duplicate_folder_groups", [], |row| row.get(0))
        .unwrap();

    refresh_duplicate_folder_groups(&conn);
    let second_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM duplicate_folder_groups", [], |row| row.get(0))
        .unwrap();

    assert_eq!(first_count, second_count);

    delete_test_data(&temp);
}
