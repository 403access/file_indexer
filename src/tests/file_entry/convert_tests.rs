use std::fs;

use crate::modules::file_entry::convert::convert_from_dir;

use crate::tests::database::arrange::{create_file, create_temp_folder, delete_test_data};

#[test]
fn convert_from_dir_file_entry_has_correct_name() {
    let temp = create_temp_folder();
    create_file(&temp, "hello.txt");

    let entry = fs::read_dir(&temp).unwrap().next().unwrap().unwrap();
    let file_entry = convert_from_dir(entry).unwrap();

    assert_eq!(file_entry.name, "hello.txt");
    delete_test_data(&temp);
}

#[test]
fn convert_from_dir_file_entry_has_correct_path() {
    let temp = create_temp_folder();
    create_file(&temp, "hello.txt");

    let entry = fs::read_dir(&temp).unwrap().next().unwrap().unwrap();
    let file_entry = convert_from_dir(entry).unwrap();

    let expected_path = temp.join("hello.txt");
    assert_eq!(file_entry.path, Some(expected_path.to_string_lossy().into_owned()));
    delete_test_data(&temp);
}

#[test]
fn convert_from_dir_file_entry_has_correct_size() {
    let temp = create_temp_folder();
    create_file(&temp, "hello");

    let entry = fs::read_dir(&temp).unwrap().next().unwrap().unwrap();
    let file_entry = convert_from_dir(entry).unwrap();

    assert_eq!(file_entry.size, 5);
    delete_test_data(&temp);
}

#[test]
fn convert_from_dir_file_flags_are_correct_for_file() {
    let temp = create_temp_folder();
    create_file(&temp, "test.txt");

    let entry = fs::read_dir(&temp).unwrap().next().unwrap().unwrap();
    let file_entry = convert_from_dir(entry).unwrap();

    assert!(file_entry.is_file);
    assert!(!file_entry.is_directory);
    assert!(!file_entry.is_symlink);
    delete_test_data(&temp);
}

#[test]
fn convert_from_dir_file_flags_are_correct_for_directory() {
    let temp = create_temp_folder();
    let dir_path = temp.join("subdir");
    fs::create_dir(&dir_path).unwrap();

    let entry = fs::read_dir(&temp).unwrap().next().unwrap().unwrap();
    let file_entry = convert_from_dir(entry).unwrap();

    assert!(file_entry.is_directory);
    assert!(!file_entry.is_file);
    assert!(!file_entry.is_symlink);
    delete_test_data(&temp);
}

#[test]
fn convert_from_dir_file_has_hash() {
    let temp = create_temp_folder();
    create_file(&temp, "test.txt");

    let entry = fs::read_dir(&temp).unwrap().next().unwrap().unwrap();
    let file_entry = convert_from_dir(entry).unwrap();

    assert!(file_entry.hash.is_some());
    assert!(!file_entry.hash.unwrap().is_empty());
    delete_test_data(&temp);
}

#[test]
fn convert_from_dir_directory_has_no_hash() {
    let temp = create_temp_folder();
    let dir_path = temp.join("subdir");
    fs::create_dir(&dir_path).unwrap();

    let entry = fs::read_dir(&temp).unwrap().next().unwrap().unwrap();
    let file_entry = convert_from_dir(entry).unwrap();

    assert!(file_entry.hash.is_none());
    delete_test_data(&temp);
}

#[test]
fn convert_from_dir_same_file_produces_same_hash() {
    let temp = create_temp_folder();
    create_file(&temp, "identical.txt");

    let mut iter = fs::read_dir(&temp).unwrap();
    let entry1 = iter.next().unwrap().unwrap();
    let fe1 = convert_from_dir(entry1).unwrap();

    let entry2 = fs::read_dir(&temp).unwrap().next().unwrap().unwrap();
    let fe2 = convert_from_dir(entry2).unwrap();

    assert_eq!(fe1.hash, fe2.hash);
    delete_test_data(&temp);
}

#[test]
fn convert_from_dir_different_files_produce_different_hashes() {
    let temp = create_temp_folder();
    create_file(&temp, "file-a");
    create_file(&temp, "file-b");

    let entries: Vec<_> = fs::read_dir(&temp)
        .unwrap()
        .map(|e| e.unwrap())
        .collect();
    let mut hashes: Vec<String> = entries
        .into_iter()
        .map(|e| convert_from_dir(e).unwrap().hash.unwrap())
        .collect();
    hashes.sort();

    assert_eq!(hashes.len(), 2);
    assert_ne!(hashes[0], hashes[1]);
    delete_test_data(&temp);
}

#[test]
fn convert_from_dir_modified_timestamp_is_some() {
    let temp = create_temp_folder();
    create_file(&temp, "test.txt");

    let entry = fs::read_dir(&temp).unwrap().next().unwrap().unwrap();
    let file_entry = convert_from_dir(entry).unwrap();

    assert!(file_entry.modified.is_some());
    delete_test_data(&temp);
}
