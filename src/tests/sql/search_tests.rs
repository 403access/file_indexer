use std::path::PathBuf;

use crate::modules::commands::command_search_file::{OrderKind, PatternKind, TargetKind};
use crate::modules::file_entry::_types::FileEntry;
use crate::modules::sql::database::{get_connection, init_db, insert_file, insert_file_name};
use crate::modules::sql::search::search_file;

use crate::tests::database::arrange::{create_temp_folder, delete_test_data};

fn setup_db_with_data(temp: &PathBuf) {
    let mut conn = get_connection(temp.join("test.db").to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    init_db(&tx).unwrap();

    let name_id = insert_file_name(&tx, "alpha.txt").unwrap();
    insert_file(
        &tx,
        &FileEntry {
            path: Some("/test/alpha.txt".to_string()),
            name: "alpha.txt".to_string(),
            size: 10,
            created: None,
            modified: None,
            accessed: None,
            hash: Some("hash1".to_string()),
            is_directory: false,
            is_file: true,
            is_symlink: false,
            parent_path: None,
        },
        name_id,
        None,
    )
    .unwrap();

    let name_id = insert_file_name(&tx, "beta.txt").unwrap();
    insert_file(
        &tx,
        &FileEntry {
            path: Some("/test/beta.txt".to_string()),
            name: "beta.txt".to_string(),
            size: 20,
            created: None,
            modified: None,
            accessed: None,
            hash: Some("hash2".to_string()),
            is_directory: false,
            is_file: true,
            is_symlink: false,
            parent_path: None,
        },
        name_id,
        None,
    )
    .unwrap();

    let name_id = insert_file_name(&tx, "alpha_dir").unwrap();
    insert_file(
        &tx,
        &FileEntry {
            path: Some("/test/alpha_dir".to_string()),
            name: "alpha_dir".to_string(),
            size: 0,
            created: None,
            modified: None,
            accessed: None,
            hash: None,
            is_directory: true,
            is_file: false,
            is_symlink: false,
            parent_path: None,
        },
        name_id,
        None,
    )
    .unwrap();

    tx.commit().unwrap();
}

fn search(temp: &PathBuf, name: &str, target: TargetKind, pattern: PatternKind, order: OrderKind) -> Vec<FileEntry> {
    let mut conn = get_connection(temp.join("test.db").to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    let results = search_file(&tx, name, target, pattern, order).unwrap();
    tx.commit().unwrap();
    results
}

#[test]
fn search_exact_match_finds_file() {
    let temp = create_temp_folder();
    setup_db_with_data(&temp);
    let results = search(&temp, "alpha.txt", TargetKind::Files, PatternKind::Exact, OrderKind::Asc);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "alpha.txt");
    delete_test_data(&temp);
}

#[test]
fn search_exact_no_match_returns_empty() {
    let temp = create_temp_folder();
    setup_db_with_data(&temp);
    let results = search(&temp, "gamma.txt", TargetKind::Files, PatternKind::Exact, OrderKind::Asc);
    assert!(results.is_empty());
    delete_test_data(&temp);
}

#[test]
fn search_starts_with_matches() {
    let temp = create_temp_folder();
    setup_db_with_data(&temp);
    let results = search(&temp, "alpha", TargetKind::Both, PatternKind::StartsWith, OrderKind::Asc);
    assert_eq!(results.len(), 2);
    let names: Vec<&str> = results.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"alpha.txt"));
    assert!(names.contains(&"alpha_dir"));
    delete_test_data(&temp);
}

#[test]
fn search_ends_with_matches() {
    let temp = create_temp_folder();
    setup_db_with_data(&temp);
    let results = search(&temp, ".txt", TargetKind::Files, PatternKind::EndsWith, OrderKind::Asc);
    assert_eq!(results.len(), 2);
    let names: Vec<&str> = results.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"alpha.txt"));
    assert!(names.contains(&"beta.txt"));
    delete_test_data(&temp);
}

#[test]
fn search_contains_matches() {
    let temp = create_temp_folder();
    setup_db_with_data(&temp);
    let results = search(&temp, "a", TargetKind::Both, PatternKind::Contains, OrderKind::Asc);
    assert!(!results.is_empty());
    for r in &results {
        assert!(r.name.contains('a'));
    }
    delete_test_data(&temp);
}

#[test]
fn search_files_only_returns_no_directories() {
    let temp = create_temp_folder();
    setup_db_with_data(&temp);
    let results = search(&temp, "alpha", TargetKind::Files, PatternKind::Contains, OrderKind::Asc);
    for r in &results {
        assert!(r.is_file);
    }
    delete_test_data(&temp);
}

#[test]
fn search_folders_only_returns_no_files() {
    let temp = create_temp_folder();
    setup_db_with_data(&temp);
    let results = search(&temp, "alpha", TargetKind::Folders, PatternKind::Contains, OrderKind::Asc);
    assert_eq!(results.len(), 1);
    assert!(results[0].is_directory);
    delete_test_data(&temp);
}

#[test]
fn search_both_returns_files_and_folders() {
    let temp = create_temp_folder();
    setup_db_with_data(&temp);
    let results = search(&temp, "alpha", TargetKind::Both, PatternKind::Contains, OrderKind::Asc);
    assert_eq!(results.len(), 2);
    let has_file = results.iter().any(|e| e.is_file);
    let has_dir = results.iter().any(|e| e.is_directory);
    assert!(has_file);
    assert!(has_dir);
    delete_test_data(&temp);
}

#[test]
fn search_asc_order_is_sorted() {
    let temp = create_temp_folder();
    setup_db_with_data(&temp);
    let results = search(&temp, ".txt", TargetKind::Files, PatternKind::EndsWith, OrderKind::Asc);
    assert_eq!(results.len(), 2);
    assert!(results[0].name <= results[1].name);
    delete_test_data(&temp);
}

#[test]
fn search_desc_order_is_sorted() {
    let temp = create_temp_folder();
    setup_db_with_data(&temp);
    let results = search(&temp, ".txt", TargetKind::Files, PatternKind::EndsWith, OrderKind::Desc);
    assert_eq!(results.len(), 2);
    assert!(results[0].name >= results[1].name);
    delete_test_data(&temp);
}
