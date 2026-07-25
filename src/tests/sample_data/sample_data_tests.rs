use std::path::Path;

use crate::modules::commands::command_index_files::index_directory;
use crate::modules::commands::command_search_file::{OrderKind, PatternKind, TargetKind};
use crate::modules::sql::database::get_connection;
use crate::modules::sql::search::search_file;

const SAMPLE_DIR: &str = "src/tests/data/sample-directory";

fn sample_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE_DIR)
}

fn db_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("test_sample_{}.db", uuid::Uuid::new_v4()))
}

fn search(db: &std::path::Path, name: &str, target: TargetKind, pattern: PatternKind) -> Vec<crate::modules::file_entry::_types::FileEntry> {
    let mut conn = get_connection(db.to_str().unwrap()).unwrap();
    let tx = conn.transaction().unwrap();
    let results = search_file(&tx, name, target, pattern, OrderKind::Asc).unwrap();
    tx.commit().unwrap();
    results
}

fn count_where(db: &std::path::Path, where_clause: &str) -> i64 {
    let conn = get_connection(db.to_str().unwrap()).unwrap();
    let sql = format!("SELECT COUNT(*) FROM files {}", where_clause);
    conn.query_row(&sql, [], |row| row.get(0)).unwrap()
}

/// Golden test: assert the full structure of src/tests/data/sample-directory/.
///
/// When someone adds, removes, or renames a file here, this test breaks
/// and the failure message tells them exactly what changed — update the
/// expectations below to match.
///
/// ```text
/// src/tests/data/sample-directory/
/// ├── .DS_Store
/// ├── file-a
/// ├── file-b
/// ├── folder-a/
/// │   ├── file-a-1
/// │   └── file-a-2
/// ├── folder-b/
/// │   └── file-b-1
/// ├── folder-c/
/// │   ├── file-a-1
/// │   ├── file-c-1
/// │   ├── file-c-2
/// │   ├── file-c-3
/// │   └── folder-d/
/// │       ├── folder-a/
/// │       │   ├── file-a-1
/// │       │   └── file-a-2
/// │       └── folder-b/
/// │           └── file-b-1
/// ├── folder-e/
/// │   ├── folder-a/
/// │   │   ├── file-a-1
/// │   │   └── file-a-2
/// │   ├── folder-b/
/// │   │   └── file-b-1
/// │   └── folder-c/
/// │       ├── file-c-1
/// │       └── file-c-2
/// └── folder-f/
///     ├── file-a
///     ├── file-b
///     └── file-f
/// ```
#[test]
fn sample_directory_matches_expected_structure() {
    let root = sample_path();
    let db = db_path();

    index_directory(db.to_str().unwrap(), root.to_str().unwrap()).unwrap();

    // --- aggregate counts (includes .DS_Store) ---
    assert_eq!(count_where(&db, ""), 32, "total entry count changed");
    assert_eq!(count_where(&db, "WHERE is_file = 1"), 21, "file count changed (includes .DS_Store)");
    assert_eq!(count_where(&db, "WHERE is_directory = 1"), 11, "directory count changed");

    // --- names that appear exactly twice ---
    let file_a = search(&db, "file-a", TargetKind::Files, PatternKind::Exact);
    assert_eq!(file_a.len(), 2, "file-a should appear twice (root + folder-f)");
    assert!(file_a.iter().all(|e| e.is_file));

    let folder_c = search(&db, "folder-c", TargetKind::Folders, PatternKind::Exact);
    assert_eq!(folder_c.len(), 2, "folder-c should appear twice (root + folder-e)");
    assert!(folder_c.iter().all(|e| e.is_directory));

    // --- names that appear exactly three times ---
    let folder_a = search(&db, "folder-a", TargetKind::Folders, PatternKind::Exact);
    assert_eq!(folder_a.len(), 3, "folder-a should appear three times (root + folder-c/folder-d + folder-e)");

    let file_b1 = search(&db, "file-b-1", TargetKind::Files, PatternKind::Exact);
    assert_eq!(file_b1.len(), 3, "file-b-1 should appear three times");

    // --- names that appear exactly four times ---
    let file_a1 = search(&db, "file-a-1", TargetKind::Files, PatternKind::Exact);
    assert_eq!(file_a1.len(), 4, "file-a-1 should appear four times");

    // --- names that appear exactly once ---
    let file_f = search(&db, "file-f", TargetKind::Files, PatternKind::Exact);
    assert_eq!(file_f.len(), 1, "file-f should appear once (folder-f only)");
    assert_eq!(file_f[0].name, "file-f");

    let file_c3 = search(&db, "file-c-3", TargetKind::Files, PatternKind::Exact);
    assert_eq!(file_c3.len(), 1, "file-c-3 should appear once (folder-c only)");

    let _ = std::fs::remove_file(&db);
}
