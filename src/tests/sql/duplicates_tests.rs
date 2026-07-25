use std::path::PathBuf;

use crate::modules::file_entry::_types::FileEntry;
use crate::modules::sql::database::{get_connection, init_db, insert_file, insert_file_name};
use crate::modules::sql::duplicates::get_duplicates;

use crate::tests::database::arrange::{create_temp_folder, delete_test_data};

fn setup_db(temp: &PathBuf) {
    let mut conn = get_connection(temp.join("test.db").to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    init_db(&tx).unwrap();
    tx.commit().unwrap();
}

fn insert_test_file(temp: &PathBuf, name: &str, path: &str, hash: &str) {
    let mut conn = get_connection(temp.join("test.db").to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    let name_id = insert_file_name(&tx, name).unwrap();
    insert_file(
        &tx,
        &FileEntry {
            path: Some(path.to_string()),
            name: name.to_string(),
            size: 10,
            created: None,
            modified: None,
            accessed: None,
            hash: Some(hash.to_string()),
            is_directory: false,
            is_file: true,
            is_symlink: false,
        },
        name_id,
    )
    .unwrap();
    tx.commit().unwrap();
}

#[test]
fn get_duplicates_returns_empty_when_no_duplicates() {
    let temp = create_temp_folder();
    setup_db(&temp);
    insert_test_file(&temp, "a.txt", "/test/a.txt", "hash_a");
    insert_test_file(&temp, "b.txt", "/test/b.txt", "hash_b");

    let mut conn = get_connection(temp.join("test.db").to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    let results = get_duplicates(&tx, None).unwrap();
    assert!(results.is_empty());
    tx.commit().unwrap();

    delete_test_data(&temp);
}

#[test]
fn get_duplicates_returns_matching_files() {
    let temp = create_temp_folder();
    setup_db(&temp);
    insert_test_file(&temp, "a.txt", "/test/a.txt", "same_hash");
    insert_test_file(&temp, "b.txt", "/test/b.txt", "same_hash");
    insert_test_file(&temp, "c.txt", "/test/c.txt", "unique_hash");

    let mut conn = get_connection(temp.join("test.db").to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    let results = get_duplicates(&tx, None).unwrap();
    assert_eq!(results.len(), 2);
    let names: Vec<&str> = results.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"a.txt"));
    assert!(names.contains(&"b.txt"));
    tx.commit().unwrap();

    delete_test_data(&temp);
}

#[test]
fn get_duplicates_with_limit() {
    let temp = create_temp_folder();
    setup_db(&temp);
    insert_test_file(&temp, "a.txt", "/test/a.txt", "hash1");
    insert_test_file(&temp, "b.txt", "/test/b.txt", "hash1");
    insert_test_file(&temp, "c.txt", "/test/c.txt", "hash2");
    insert_test_file(&temp, "d.txt", "/test/d.txt", "hash2");

    let mut conn = get_connection(temp.join("test.db").to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    let results = get_duplicates(&tx, Some(1)).unwrap();
    assert_eq!(results.len(), 1);
    tx.commit().unwrap();

    delete_test_data(&temp);
}

#[test]
fn get_duplicates_three_way_duplicate() {
    let temp = create_temp_folder();
    setup_db(&temp);
    insert_test_file(&temp, "a.txt", "/test/a.txt", "triplicate");
    insert_test_file(&temp, "b.txt", "/test/b.txt", "triplicate");
    insert_test_file(&temp, "c.txt", "/test/c.txt", "triplicate");

    let mut conn = get_connection(temp.join("test.db").to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    let results = get_duplicates(&tx, None).unwrap();
    assert_eq!(results.len(), 3);
    tx.commit().unwrap();

    delete_test_data(&temp);
}

#[test]
fn get_duplicates_excludes_unique_hashes() {
    let temp = create_temp_folder();
    setup_db(&temp);
    insert_test_file(&temp, "a.txt", "/test/a.txt", "unique");
    insert_test_file(&temp, "b.txt", "/test/b.txt", "duplicate");
    insert_test_file(&temp, "c.txt", "/test/c.txt", "duplicate");

    let mut conn = get_connection(temp.join("test.db").to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    let results = get_duplicates(&tx, None).unwrap();
    let names: Vec<&str> = results.iter().map(|e| e.name.as_str()).collect();
    assert!(!names.contains(&"a.txt"));
    tx.commit().unwrap();

    delete_test_data(&temp);
}
