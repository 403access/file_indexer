use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::modules::commands::command_index_files::index_directory;
use crate::modules::sql::database::get_connection;
use crate::modules::sql::duplicates::get_duplicates;

use crate::tests::database::arrange::{create_temp_folder, delete_test_data};

fn create_identical_file(dir: &PathBuf, name: &str, content: &[u8]) {
    let mut file = fs::File::create(dir.join(name)).unwrap();
    file.write_all(content).unwrap();
}

fn setup_and_index(temp: &PathBuf) {
    let db_path = temp.join("file_index.db");
    index_directory(db_path.to_str().unwrap(), temp.to_str().unwrap()).unwrap();
}

fn count_duplicates(temp: &PathBuf) -> usize {
    let mut conn = get_connection(temp.join("file_index.db").to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    let results = get_duplicates(&tx, None).unwrap();
    let count = results.len();
    tx.commit().unwrap();
    count
}

fn get_duplicate_names(temp: &PathBuf) -> Vec<String> {
    let mut conn = get_connection(temp.join("file_index.db").to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    let results = get_duplicates(&tx, None).unwrap();
    let names: Vec<String> = results.iter().map(|e| e.name.clone()).collect();
    tx.commit().unwrap();
    names
}

#[test]
fn no_duplicates_when_all_files_unique() {
    let temp = create_temp_folder();
    create_identical_file(&temp, "a.txt", b"content-a");
    create_identical_file(&temp, "b.txt", b"content-b");
    create_identical_file(&temp, "c.txt", b"content-c");
    setup_and_index(&temp);

    assert_eq!(count_duplicates(&temp), 0);

    delete_test_data(&temp);
}

#[test]
fn duplicate_files_with_same_content() {
    let temp = create_temp_folder();
    let content = b"identical content";
    create_identical_file(&temp, "copy-a.txt", content);
    create_identical_file(&temp, "copy-b.txt", content);
    create_identical_file(&temp, "copy-c.txt", content);
    setup_and_index(&temp);

    assert_eq!(count_duplicates(&temp), 3);

    let names = get_duplicate_names(&temp);
    assert!(names.contains(&"copy-a.txt".to_string()));
    assert!(names.contains(&"copy-b.txt".to_string()));
    assert!(names.contains(&"copy-c.txt".to_string()));

    delete_test_data(&temp);
}

#[test]
fn only_duplicates_are_returned() {
    let temp = create_temp_folder();
    let content = b"same content";
    create_identical_file(&temp, "same-1.txt", content);
    create_identical_file(&temp, "same-2.txt", content);
    create_identical_file(&temp, "different.txt", b"something else entirely");
    setup_and_index(&temp);

    let names = get_duplicate_names(&temp);
    assert!(!names.contains(&"different.txt".to_string()));
    assert_eq!(names.len(), 2);

    delete_test_data(&temp);
}

#[test]
fn duplicates_in_subdirectories() {
    let temp = create_temp_folder();
    let content = b"duplicate across dirs";
    create_identical_file(&temp, "root-file.txt", content);
    let sub = temp.join("subdir");
    fs::create_dir(&sub).unwrap();
    create_identical_file(&sub, "sub-file.txt", content);
    setup_and_index(&temp);

    assert_eq!(count_duplicates(&temp), 2);

    let mut conn = get_connection(temp.join("file_index.db").to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    let results = get_duplicates(&tx, None).unwrap();
    for entry in &results {
        assert!(entry.path.is_some());
    }
    tx.commit().unwrap();

    delete_test_data(&temp);
}

#[test]
fn duplicates_limit_works() {
    let temp = create_temp_folder();
    let content = b"limited content";
    create_identical_file(&temp, "dup-a.txt", content);
    create_identical_file(&temp, "dup-b.txt", content);
    create_identical_file(&temp, "dup-c.txt", content);
    create_identical_file(&temp, "dup-d.txt", content);
    setup_and_index(&temp);

    let mut conn = get_connection(temp.join("file_index.db").to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    let results = get_duplicates(&tx, Some(2)).unwrap();
    assert_eq!(results.len(), 2);
    tx.commit().unwrap();

    delete_test_data(&temp);
}

#[test]
fn three_way_duplicates_all_returned() {
    let temp = create_temp_folder();
    let content = b"triple duplicate";
    create_identical_file(&temp, "triple-1.txt", content);
    create_identical_file(&temp, "triple-2.txt", content);
    create_identical_file(&temp, "triple-3.txt", content);
    setup_and_index(&temp);

    assert_eq!(count_duplicates(&temp), 3);

    delete_test_data(&temp);
}
