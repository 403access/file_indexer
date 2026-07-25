use crate::modules::file_entry::_types::FileEntry;
use crate::modules::sql::database::{
    get_connection, init_db, insert_file, insert_file_name,
};
use crate::modules::sql::search::search_file;
use crate::modules::commands::command_search_file::{OrderKind, PatternKind, TargetKind};

use crate::tests::database::arrange::{create_temp_folder, delete_test_data};

fn setup_and_index(temp: &std::path::PathBuf) {
    let mut conn = get_connection(temp.join("test.db").to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    init_db(&tx).unwrap();

    let entries = vec![
        ("file-a", "/root/file-a", 10, Some("hash_a"), false, true),
        ("file-b", "/root/file-b", 20, Some("hash_b"), false, true),
        ("file-c-1", "/root/folder-c/file-c-1", 30, Some("hash_c1"), false, true),
        ("file-c-2", "/root/folder-c/file-c-2", 40, Some("hash_c2"), false, true),
        ("file-a-1", "/root/folder-c/file-a-1", 50, Some("hash_a1"), false, true),
        ("file-b-1", "/root/folder-b/file-b-1", 60, Some("hash_b1"), false, true),
        ("folder-a", "/root/folder-a", 0, None, true, false),
        ("folder-b", "/root/folder-b", 0, None, true, false),
        ("folder-c", "/root/folder-c", 0, None, true, false),
    ];

    for (name, path, size, hash, is_dir, is_file) in entries {
        let name_id = insert_file_name(&tx, name).unwrap();
        insert_file(
            &tx,
            &FileEntry {
                path: Some(path.to_string()),
                name: name.to_string(),
                size,
                created: None,
                modified: None,
                accessed: None,
                hash: hash.map(|h| h.to_string()),
                is_directory: is_dir,
                is_file,
                is_symlink: false,
            },
            name_id,
        )
        .unwrap();
    }

    tx.commit().unwrap();
}

fn search(temp: &std::path::PathBuf, name: &str, target: TargetKind, pattern: PatternKind, order: OrderKind) -> Vec<FileEntry> {
    let mut conn = get_connection(temp.join("test.db").to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    let results = search_file(&tx, name, target, pattern, order).unwrap();
    tx.commit().unwrap();
    results
}

#[test]
fn index_then_search_exact_finds_file() {
    let temp = create_temp_folder();
    setup_and_index(&temp);
    let results = search(&temp, "file-a", TargetKind::Files, PatternKind::Exact, OrderKind::Asc);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "file-a");
    delete_test_data(&temp);
}

#[test]
fn index_then_search_starts_with_finds_folders() {
    let temp = create_temp_folder();
    setup_and_index(&temp);
    let results = search(&temp, "folder", TargetKind::Folders, PatternKind::StartsWith, OrderKind::Asc);
    assert_eq!(results.len(), 3);
    for entry in &results {
        assert!(entry.name.starts_with("folder"));
        assert!(entry.is_directory);
    }
    delete_test_data(&temp);
}

#[test]
fn index_then_search_ends_with_finds_files() {
    let temp = create_temp_folder();
    setup_and_index(&temp);
    let results = search(&temp, "-1", TargetKind::Files, PatternKind::EndsWith, OrderKind::Asc);
    assert!(!results.is_empty());
    for entry in &results {
        assert!(entry.name.ends_with("-1"));
    }
    delete_test_data(&temp);
}

#[test]
fn index_then_search_contains_finds_multiple() {
    let temp = create_temp_folder();
    setup_and_index(&temp);
    let results = search(&temp, "file", TargetKind::Both, PatternKind::Contains, OrderKind::Asc);
    assert!(!results.is_empty());
    for entry in &results {
        assert!(entry.name.contains("file"));
    }
    delete_test_data(&temp);
}

#[test]
fn index_then_search_files_only_excludes_directories() {
    let temp = create_temp_folder();
    setup_and_index(&temp);
    let results = search(&temp, "folder", TargetKind::Files, PatternKind::Contains, OrderKind::Asc);
    assert!(results.is_empty());
    delete_test_data(&temp);
}

#[test]
fn index_then_search_folders_only_excludes_files() {
    let temp = create_temp_folder();
    setup_and_index(&temp);
    let results = search(&temp, "file", TargetKind::Folders, PatternKind::Contains, OrderKind::Asc);
    assert!(results.is_empty());
    delete_test_data(&temp);
}
