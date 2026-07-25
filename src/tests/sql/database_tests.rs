use std::path::PathBuf;

use crate::modules::file_entry::_types::FileEntry;
use crate::modules::sql::database::{
    get_connection, init_db, insert_file, insert_file_name,
};

use crate::tests::database::arrange::{create_temp_folder, delete_test_data};

fn setup_db(temp: &PathBuf) -> rusqlite::Connection {
    let db_path = temp.join("test.db");
    let mut conn = get_connection(db_path.to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    init_db(&tx).unwrap();
    tx.commit().unwrap();
    conn
}

#[test]
fn init_db_creates_file_names_table() {
    let temp = create_temp_folder();
    let conn = setup_db(&temp);

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM file_names", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);

    delete_test_data(&temp);
}

#[test]
fn init_db_creates_files_table() {
    let temp = create_temp_folder();
    let conn = setup_db(&temp);

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);

    delete_test_data(&temp);
}

#[test]
fn init_db_creates_hash_index() {
    let temp = create_temp_folder();
    let conn = setup_db(&temp);

    let hash_idx: String = conn
        .query_row(
            "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_files_hash'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(hash_idx, "idx_files_hash");

    delete_test_data(&temp);
}

#[test]
fn init_db_creates_path_index() {
    let temp = create_temp_folder();
    let conn = setup_db(&temp);

    let path_idx: String = conn
        .query_row(
            "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_files_path'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(path_idx, "idx_files_path");

    delete_test_data(&temp);
}

#[test]
fn insert_file_name_returns_id() {
    let temp = create_temp_folder();
    let mut conn = setup_db(&temp);

    let tx = conn.transaction().unwrap();
    let id = insert_file_name(&tx, "test.txt").unwrap();
    assert!(id > 0);
    tx.commit().unwrap();

    delete_test_data(&temp);
}

#[test]
fn insert_file_name_duplicate_returns_error() {
    let temp = create_temp_folder();
    let mut conn = setup_db(&temp);

    let tx = conn.transaction().unwrap();
    let _ = insert_file_name(&tx, "test.txt").unwrap();
    let result = insert_file_name(&tx, "test.txt");
    assert!(result.is_err());
    tx.commit().unwrap();

    delete_test_data(&temp);
}

#[test]
fn insert_file_returns_id() {
    let temp = create_temp_folder();
    let mut conn = setup_db(&temp);

    let tx = conn.transaction().unwrap();
    let name_id = insert_file_name(&tx, "file.txt").unwrap();

    let file = FileEntry {
        path: Some("/test/file.txt".to_string()),
        name: "file.txt".to_string(),
        size: 100,
        created: None,
        modified: Some(1234567890),
        accessed: None,
        hash: Some("abc123".to_string()),
        is_directory: false,
        is_file: true,
        is_symlink: false,
    };

    let file_id = insert_file(&tx, &file, name_id).unwrap();
    assert!(file_id > 0);
    tx.commit().unwrap();

    delete_test_data(&temp);
}

#[test]
fn insert_file_duplicate_path_returns_error() {
    let temp = create_temp_folder();
    let mut conn = setup_db(&temp);

    let tx = conn.transaction().unwrap();
    let name_id = insert_file_name(&tx, "file.txt").unwrap();

    let file = FileEntry {
        path: Some("/test/file.txt".to_string()),
        name: "file.txt".to_string(),
        size: 100,
        created: None,
        modified: Some(1234567890),
        accessed: None,
        hash: Some("abc123".to_string()),
        is_directory: false,
        is_file: true,
        is_symlink: false,
    };

    let _ = insert_file(&tx, &file, name_id).unwrap();
    let result = insert_file(&tx, &file, name_id);
    assert!(result.is_err());
    tx.commit().unwrap();

    delete_test_data(&temp);
}

#[test]
fn insert_file_stores_correct_metadata() {
    let temp = create_temp_folder();
    let mut conn = setup_db(&temp);

    let tx = conn.transaction().unwrap();
    let name_id = insert_file_name(&tx, "file.txt").unwrap();

    let file = FileEntry {
        path: Some("/test/file.txt".to_string()),
        name: "file.txt".to_string(),
        size: 2048,
        created: None,
        modified: Some(9999999),
        accessed: None,
        hash: Some("def456".to_string()),
        is_directory: false,
        is_file: true,
        is_symlink: false,
    };

    let _ = insert_file(&tx, &file, name_id).unwrap();
    tx.commit().unwrap();

    let conn2 = get_connection(temp.join("test.db").to_str().unwrap()).unwrap();
    let mut stmt = conn2
        .prepare("SELECT size, modified, hash, is_file FROM files WHERE path = ?1")
        .unwrap();
    let row = stmt
        .query_row(["/test/file.txt"], |row| {
            Ok((
                row.get::<_, i64>("size").unwrap(),
                row.get::<_, f64>("modified").unwrap(),
                row.get::<_, String>("hash").unwrap(),
                row.get::<_, i32>("is_file").unwrap(),
            ))
        })
        .unwrap();

    assert_eq!(row.0, 2048);
    assert_eq!(row.1, 9999999.0);
    assert_eq!(row.2, "def456");
    assert_eq!(row.3, 1);

    delete_test_data(&temp);
}

#[test]
fn insert_directory_flags_are_stored() {
    let temp = create_temp_folder();
    let mut conn = setup_db(&temp);

    let tx = conn.transaction().unwrap();
    let name_id = insert_file_name(&tx, "mydir").unwrap();

    let file = FileEntry {
        path: Some("/test/mydir".to_string()),
        name: "mydir".to_string(),
        size: 0,
        created: None,
        modified: None,
        accessed: None,
        hash: None,
        is_directory: true,
        is_file: false,
        is_symlink: false,
    };

    let _ = insert_file(&tx, &file, name_id).unwrap();
    tx.commit().unwrap();

    let conn2 = get_connection(temp.join("test.db").to_str().unwrap()).unwrap();
    let mut stmt = conn2
        .prepare("SELECT is_directory, is_file, is_symlink FROM files WHERE path = ?1")
        .unwrap();
    let row = stmt
        .query_row(["/test/mydir"], |row| {
            Ok((
                row.get::<_, i32>("is_directory").unwrap(),
                row.get::<_, i32>("is_file").unwrap(),
                row.get::<_, i32>("is_symlink").unwrap(),
            ))
        })
        .unwrap();

    assert_eq!(row.0, 1);
    assert_eq!(row.1, 0);
    assert_eq!(row.2, 0);

    delete_test_data(&temp);
}

#[test]
fn init_db_is_idempotent() {
    let temp = create_temp_folder();
    let mut conn = setup_db(&temp);

    let tx = conn.transaction().unwrap();
    let result = init_db(&tx);
    assert!(result.is_ok());
    tx.commit().unwrap();

    delete_test_data(&temp);
}
