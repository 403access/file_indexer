use std::path::Path;

use crate::modules::commands::command_index_files::index_directory;
use crate::modules::commands::command_search_file::{OrderKind, PatternKind, TargetKind};
use crate::modules::sql::database::get_connection;
use crate::modules::sql::search::search_file;

use super::super::database::arrange::{create_test_data, delete_test_data};

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

/// Golden test: the real src/tests/data/sample-directory/ and create_test_data()
/// must produce the same indexed structure. When someone changes one but not the
/// other, this test breaks. Update create_test_data() in arrange.rs to match.
#[test]
fn sample_directory_matches_create_test_data() {
    // Index the real on-disk sample directory
    let root = sample_path();
    let real_db = db_path();
    index_directory(real_db.to_str().unwrap(), root.to_str().unwrap(), None, None).unwrap();

    // Index the synthetic copy from arrange.rs
    let temp = create_test_data();
    let synth_db = db_path();
    index_directory(synth_db.to_str().unwrap(), temp.to_str().unwrap(), None, None).unwrap();

    // The real directory has .DS_Store; the synthetic copy does not.
    // Account for that one extra file.
    let real_total = count_where(&real_db, "");
    let synth_total = count_where(&synth_db, "");
    assert_eq!(real_total, synth_total + 1, "real dir should have exactly 1 more entry than create_test_data() (.DS_Store)");

    let real_files = count_where(&real_db, "WHERE is_file = 1");
    let synth_files = count_where(&synth_db, "WHERE is_file = 1");
    assert_eq!(real_files, synth_files + 1, "real dir should have 1 more file (.DS_Store)");

    let real_dirs = count_where(&real_db, "WHERE is_directory = 1");
    let synth_dirs = count_where(&synth_db, "WHERE is_directory = 1");
    assert_eq!(real_dirs, synth_dirs, "directory count must match exactly");

    // Search for names that appear multiple times — counts must agree
    for name in &["file-a", "file-b", "file-a-1", "file-b-1", "file-c-1", "folder-a", "folder-c"] {
        let real = search(&real_db, name, TargetKind::Both, PatternKind::Exact);
        let synth = search(&synth_db, name, TargetKind::Both, PatternKind::Exact);
        assert_eq!(
            real.len(),
            synth.len(),
            "search for '{}' returned different counts: real={}, synthetic={}",
            name, real.len(), synth.len()
        );
    }

    let _ = std::fs::remove_file(&real_db);
    let _ = std::fs::remove_file(&synth_db);
    delete_test_data(&temp);
}
