use crate::modules::commands::command_index_files::index_directory;
use crate::modules::commands::command_search_file::{OrderKind, PatternKind, TargetKind};
use crate::modules::sql::database::get_connection;
use crate::modules::sql::search::search_file;

use crate::tests::database::arrange::{
    create_database, create_test_data, delete_database, delete_test_data,
};

fn search(temp: &std::path::Path, name: &str, target: TargetKind, pattern: PatternKind, order: OrderKind) -> Vec<crate::modules::file_entry::_types::FileEntry> {
    let mut conn = get_connection(temp.join("file_index.db").to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    let results = search_file(&tx, name, target, pattern, order).unwrap();
    tx.commit().unwrap();
    results
}

#[test]
fn index_then_search_exact_finds_file() {
    let temp = create_test_data();
    let db_path = temp.join("file_index.db");
    create_database(&db_path);

    index_directory(db_path.to_str().unwrap(), temp.to_str().unwrap()).unwrap();

    let results = search(&temp, "file-a", TargetKind::Files, PatternKind::Exact, OrderKind::Asc);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "file-a");
    assert!(results[0].is_file);

    delete_database(&db_path);
    delete_test_data(&temp);
}

#[test]
fn index_then_search_starts_with_finds_folders() {
    let temp = create_test_data();
    let db_path = temp.join("file_index.db");
    create_database(&db_path);

    index_directory(db_path.to_str().unwrap(), temp.to_str().unwrap()).unwrap();

    let results = search(&temp, "folder", TargetKind::Folders, PatternKind::StartsWith, OrderKind::Asc);
    assert!(!results.is_empty());
    for entry in &results {
        assert!(entry.name.starts_with("folder"));
        assert!(entry.is_directory);
    }

    delete_database(&db_path);
    delete_test_data(&temp);
}

#[test]
fn index_then_search_ends_with_finds_files() {
    let temp = create_test_data();
    let db_path = temp.join("file_index.db");
    create_database(&db_path);

    index_directory(db_path.to_str().unwrap(), temp.to_str().unwrap()).unwrap();

    let results = search(&temp, "-1", TargetKind::Files, PatternKind::EndsWith, OrderKind::Asc);
    assert!(!results.is_empty());
    for entry in &results {
        assert!(entry.name.ends_with("-1"));
        assert!(entry.is_file);
    }

    delete_database(&db_path);
    delete_test_data(&temp);
}

#[test]
fn index_then_search_contains_finds_multiple() {
    let temp = create_test_data();
    let db_path = temp.join("file_index.db");
    create_database(&db_path);

    index_directory(db_path.to_str().unwrap(), temp.to_str().unwrap()).unwrap();

    let results = search(&temp, "file", TargetKind::Both, PatternKind::Contains, OrderKind::Asc);
    assert!(!results.is_empty());
    for entry in &results {
        assert!(entry.name.contains("file"));
    }

    delete_database(&db_path);
    delete_test_data(&temp);
}

#[test]
fn index_then_search_files_only_excludes_directories() {
    let temp = create_test_data();
    let db_path = temp.join("file_index.db");
    create_database(&db_path);

    index_directory(db_path.to_str().unwrap(), temp.to_str().unwrap()).unwrap();

    let results = search(&temp, "folder", TargetKind::Files, PatternKind::Contains, OrderKind::Asc);
    assert!(results.is_empty());

    delete_database(&db_path);
    delete_test_data(&temp);
}

#[test]
fn index_then_search_folders_only_excludes_files() {
    let temp = create_test_data();
    let db_path = temp.join("file_index.db");
    create_database(&db_path);

    index_directory(db_path.to_str().unwrap(), temp.to_str().unwrap()).unwrap();

    let results = search(&temp, "file", TargetKind::Folders, PatternKind::Contains, OrderKind::Asc);
    assert!(results.is_empty());

    delete_database(&db_path);
    delete_test_data(&temp);
}

#[test]
fn index_then_search_asc_order_is_sorted() {
    let temp = create_test_data();
    let db_path = temp.join("file_index.db");
    create_database(&db_path);

    index_directory(db_path.to_str().unwrap(), temp.to_str().unwrap()).unwrap();

    let results = search(&temp, "file", TargetKind::Files, PatternKind::Contains, OrderKind::Asc);
    assert!(results.len() >= 2);
    for window in results.windows(2) {
        assert!(window[0].name <= window[1].name);
    }

    delete_database(&db_path);
    delete_test_data(&temp);
}

#[test]
fn index_then_search_desc_order_is_sorted() {
    let temp = create_test_data();
    let db_path = temp.join("file_index.db");
    create_database(&db_path);

    index_directory(db_path.to_str().unwrap(), temp.to_str().unwrap()).unwrap();

    let results = search(&temp, "file", TargetKind::Files, PatternKind::Contains, OrderKind::Desc);
    assert!(results.len() >= 2);
    for window in results.windows(2) {
        assert!(window[0].name >= window[1].name);
    }

    delete_database(&db_path);
    delete_test_data(&temp);
}

#[test]
fn index_produces_correct_file_count() {
    let temp = create_test_data();
    let db_path = temp.join("file_index.db");
    create_database(&db_path);

    index_directory(db_path.to_str().unwrap(), temp.to_str().unwrap()).unwrap();

    let conn = get_connection(db_path.to_str().unwrap()).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
        .unwrap();
    assert!(count > 0);

    delete_database(&db_path);
    delete_test_data(&temp);
}

#[test]
fn index_produces_correct_directory_count() {
    let temp = create_test_data();
    let db_path = temp.join("file_index.db");
    create_database(&db_path);

    index_directory(db_path.to_str().unwrap(), temp.to_str().unwrap()).unwrap();

    let conn = get_connection(db_path.to_str().unwrap()).unwrap();
    let dir_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM files WHERE is_directory = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(dir_count > 0);

    delete_database(&db_path);
    delete_test_data(&temp);
}
