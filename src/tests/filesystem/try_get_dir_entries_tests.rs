use crate::modules::search_files::try_get_dir_entries::try_get_dir_entries;
use crate::modules::file_entry::sort::SortOrder;

use crate::tests::database::arrange::{create_file, create_temp_folder, delete_test_data};

use std::fs;

#[test]
fn try_get_dir_entries_returns_files() {
    let temp = create_temp_folder();
    create_file(&temp, "file-a.txt");
    create_file(&temp, "file-b.txt");

    let path = format!("{}/", temp.to_str().unwrap());
    let entries = try_get_dir_entries(&path, None).unwrap();

    assert_eq!(entries.len(), 2);
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"file-a.txt"));
    assert!(names.contains(&"file-b.txt"));
    delete_test_data(&temp);
}

#[test]
fn try_get_dir_entries_returns_directories() {
    let temp = create_temp_folder();
    fs::create_dir(temp.join("subdir")).unwrap();

    let path = format!("{}/", temp.to_str().unwrap());
    let entries = try_get_dir_entries(&path, None).unwrap();

    assert_eq!(entries.len(), 1);
    assert!(entries[0].is_directory);
    delete_test_data(&temp);
}

#[test]
fn try_get_dir_entries_with_sort_order() {
    let temp = create_temp_folder();
    create_file(&temp, "c.txt");
    create_file(&temp, "a.txt");
    create_file(&temp, "b.txt");

    let path = format!("{}/", temp.to_str().unwrap());
    let entries = try_get_dir_entries(&path, Some(SortOrder::AaBbCc)).unwrap();

    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["a.txt", "b.txt", "c.txt"]);
    delete_test_data(&temp);
}

#[test]
fn try_get_dir_entries_empty_directory() {
    let temp = create_temp_folder();

    let path = format!("{}/", temp.to_str().unwrap());
    let entries = try_get_dir_entries(&path, None).unwrap();

    assert!(entries.is_empty());
    delete_test_data(&temp);
}

#[test]
fn try_get_dir_entries_rejects_empty_path() {
    let result = try_get_dir_entries("", None);
    assert!(result.is_err());
}

#[test]
fn try_get_dir_entries_rejects_relative_path() {
    let result = try_get_dir_entries("relative/path/", None);
    assert!(result.is_err());
}

#[test]
fn try_get_dir_entries_rejects_path_without_trailing_slash() {
    let temp = create_temp_folder();
    let path = temp.to_str().unwrap();
    let result = try_get_dir_entries(path, None);
    assert!(result.is_err());
    delete_test_data(&temp);
}

#[test]
fn try_get_dir_entries_rejects_nonexistent_path() {
    let result = try_get_dir_entries("/nonexistent/path/", None);
    assert!(result.is_err());
}

#[test]
fn try_get_dir_entries_returns_file_size() {
    let temp = create_temp_folder();
    create_file(&temp, "sized.txt");

    let path = format!("{}/", temp.to_str().unwrap());
    let entries = try_get_dir_entries(&path, None).unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].size, 9);
    delete_test_data(&temp);
}

#[test]
fn try_get_dir_entries_returns_hash_for_files() {
    let temp = create_temp_folder();
    create_file(&temp, "hashable.txt");

    let path = format!("{}/", temp.to_str().unwrap());
    let entries = try_get_dir_entries(&path, None).unwrap();

    assert_eq!(entries.len(), 1);
    assert!(entries[0].hash.is_some());
    delete_test_data(&temp);
}
