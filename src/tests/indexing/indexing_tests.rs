use std::env::temp_dir;

use crate::modules::commands::command_index_files::index_directory;
use crate::modules::commands::command_search_file::{OrderKind, PatternKind, TargetKind};
use crate::modules::sql::database::get_connection;
use crate::modules::sql::search::search_file;

use crate::tests::database::arrange::{create_test_data, delete_test_data};
use uuid::Uuid;

fn db_path_for(_temp: &std::path::Path) -> std::path::PathBuf {
    temp_dir().join(format!("test_{}.db", Uuid::new_v4()))
}

fn search(_temp: &std::path::Path, db: &std::path::Path, name: &str, target: TargetKind, pattern: PatternKind, order: OrderKind) -> Vec<crate::modules::file_entry::_types::FileEntry> {
    let mut conn = get_connection(db.to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    let results = search_file(&tx, name, target, pattern, order).unwrap();
    tx.commit().unwrap();
    results
}

fn count_where(db: &std::path::Path, where_clause: &str) -> i64 {
    let conn = get_connection(db.to_str().unwrap()).unwrap();
    let sql = format!("SELECT COUNT(*) FROM files {}", where_clause);
    conn.query_row(&sql, [], |row| row.get(0)).unwrap()
}

fn count_all(db: &std::path::Path) -> i64 {
    count_where(db, "")
}

fn count_dirs(db: &std::path::Path) -> i64 {
    count_where(db, "WHERE is_directory = 1")
}

fn count_files(db: &std::path::Path) -> i64 {
    count_where(db, "WHERE is_file = 1")
}

fn setup(temp: &std::path::Path) -> std::path::PathBuf {
    let db = db_path_for(temp);
    index_directory(db.to_str().unwrap(), temp.to_str().unwrap(), None, None).unwrap();
    db
}

fn cleanup(db: &std::path::Path, temp: &std::path::PathBuf) {
    let _ = std::fs::remove_file(db);
    delete_test_data(temp);
}

// --- Full tree structure assertions ---

#[test]
fn index_full_tree_counts_all_entries() {
    let temp = create_test_data();
    let db = setup(&temp);

    // 20 files + 12 directories (including root) = 32 entries
    assert_eq!(count_all(&db), 32);

    cleanup(&db, &temp);
}

#[test]
fn index_full_tree_counts_directories() {
    let temp = create_test_data();
    let db = setup(&temp);

    assert_eq!(count_dirs(&db), 12);

    cleanup(&db, &temp);
}

#[test]
fn index_full_tree_counts_files() {
    let temp = create_test_data();
    let db = setup(&temp);

    assert_eq!(count_files(&db), 20);

    cleanup(&db, &temp);
}

// --- Search against full tree ---

#[test]
fn search_exact_file_a_finds_both_copies() {
    let temp = create_test_data();
    let db = setup(&temp);

    let results = search(&temp, &db, "file-a", TargetKind::Files, PatternKind::Exact, OrderKind::Asc);
    // file-a exists in: root, folder-f
    assert_eq!(results.len(), 2);
    for r in &results {
        assert_eq!(r.name, "file-a");
        assert!(r.is_file);
    }

    cleanup(&db, &temp);
}

#[test]
fn search_exact_folder_c_finds_both_copies() {
    let temp = create_test_data();
    let db = setup(&temp);

    let results = search(&temp, &db, "folder-c", TargetKind::Folders, PatternKind::Exact, OrderKind::Asc);
    // folder-c exists in: root, folder-e
    assert_eq!(results.len(), 2);
    for r in &results {
        assert!(r.is_directory);
    }

    cleanup(&db, &temp);
}

#[test]
fn search_starts_with_file_c_finds_all_c_files() {
    let temp = create_test_data();
    let db = setup(&temp);

    let results = search(&temp, &db, "file-c", TargetKind::Files, PatternKind::StartsWith, OrderKind::Asc);
    // file-c-1, file-c-2, file-c-3 in folder-c; file-c-1, file-c-2 in folder-e/folder-c
    assert_eq!(results.len(), 5);

    cleanup(&db, &temp);
}

#[test]
fn search_ends_with_a1_finds_all_a1_files() {
    let temp = create_test_data();
    let db = setup(&temp);

    let results = search(&temp, &db, "a-1", TargetKind::Files, PatternKind::EndsWith, OrderKind::Asc);
    // file-a-1 in: folder-a, folder-c, folder-c/folder-d/folder-a, folder-e/folder-a
    assert_eq!(results.len(), 4);

    cleanup(&db, &temp);
}

#[test]
fn search_contains_b1_finds_all_b1_files() {
    let temp = create_test_data();
    let db = setup(&temp);

    let results = search(&temp, &db, "b-1", TargetKind::Files, PatternKind::Contains, OrderKind::Asc);
    // file-b-1 in: folder-b, folder-c/folder-d/folder-b, folder-e/folder-b
    assert_eq!(results.len(), 3);

    cleanup(&db, &temp);
}

#[test]
fn search_files_only_excludes_directories() {
    let temp = create_test_data();
    let db = setup(&temp);

    let results = search(&temp, &db, "folder", TargetKind::Files, PatternKind::Contains, OrderKind::Asc);
    assert!(results.is_empty());

    cleanup(&db, &temp);
}

#[test]
fn search_folders_only_excludes_files() {
    let temp = create_test_data();
    let db = setup(&temp);

    let results = search(&temp, &db, "file", TargetKind::Folders, PatternKind::Contains, OrderKind::Asc);
    assert!(results.is_empty());

    cleanup(&db, &temp);
}

#[test]
fn search_asc_order_is_sorted() {
    let temp = create_test_data();
    let db = setup(&temp);

    let results = search(&temp, &db, "file", TargetKind::Files, PatternKind::Contains, OrderKind::Asc);
    assert!(results.len() >= 2);
    for window in results.windows(2) {
        assert!(window[0].name <= window[1].name);
    }

    cleanup(&db, &temp);
}

#[test]
fn search_desc_order_is_sorted() {
    let temp = create_test_data();
    let db = setup(&temp);

    let results = search(&temp, &db, "file", TargetKind::Files, PatternKind::Contains, OrderKind::Desc);
    assert!(results.len() >= 2);
    for window in results.windows(2) {
        assert!(window[0].name >= window[1].name);
    }

    cleanup(&db, &temp);
}

#[test]
fn search_no_match_returns_empty() {
    let temp = create_test_data();
    let db = setup(&temp);

    let results = search(&temp, &db, "nonexistent", TargetKind::Both, PatternKind::Exact, OrderKind::Asc);
    assert!(results.is_empty());

    cleanup(&db, &temp);
}

// --- Static sample-directory fixture ---

#[test]
fn index_static_sample_directory_matches_dynamic() {
    let sample_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/tests/data/sample-directory");

    let temp = create_test_data();
    let db = setup(&temp);

    // Count non-hidden entries in static fixture
    let static_file_count: usize = walkdir::WalkDir::new(&sample_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter(|e| e.file_type().is_file())
        .count();

    let indexed_file_count = count_files(&db);
    assert_eq!(static_file_count as i64, indexed_file_count);

    cleanup(&db, &temp);
}
