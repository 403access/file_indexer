use crate::{
    modules::commands::command_search_file::{OrderKind, PatternKind, TargetKind},
    modules::file_entry::{_types::FileEntry, convert::convert_from_rows},
};

/// Map API sort keys to SQL expressions (columns already in SELECT).
fn sort_sql(sort: &str) -> &'static str {
    match sort {
        "size" => "f.size",
        "modified" => "f.modified",
        "path" => "f.path",
        _ => "fn.name",
    }
}

fn pattern_and_types(
    name: &str,
    target_kind: TargetKind,
    pattern_kind: PatternKind,
) -> (String, i64, i64) {
    let is_directory = matches!(target_kind, TargetKind::Folders | TargetKind::Both);
    let is_file = matches!(target_kind, TargetKind::Files | TargetKind::Both);
    let pattern = match pattern_kind {
        PatternKind::Exact => name.to_string(),
        PatternKind::StartsWith => format!("{name}%"),
        PatternKind::EndsWith => format!("%{name}"),
        PatternKind::Contains => {
            if name.is_empty() {
                // Match everything without a pathological "%%" scan preference;
                // COUNT/LIMIT still apply via indexes on type flags when name empty.
                "%".to_string()
            } else {
                format!("%{name}%")
            }
        }
    };
    (
        pattern,
        if is_directory { 1 } else { 0 },
        if is_file { 1 } else { 0 },
    )
}

/// Count matching rows (for pagination total).
pub fn count_search_file(
    conn: &rusqlite::Connection,
    name: &str,
    target_kind: TargetKind,
    pattern_kind: PatternKind,
) -> rusqlite::Result<u64> {
    let (pattern, is_directory, is_file) = pattern_and_types(name, target_kind, pattern_kind);

    // Empty name + both types: use files table count only (fast)
    if name.is_empty() && is_directory == 1 && is_file == 1 {
        return conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0));
    }
    if name.is_empty() && is_file == 1 && is_directory == 0 {
        return conn.query_row(
            "SELECT COUNT(*) FROM files WHERE is_file = 1",
            [],
            |r| r.get(0),
        );
    }
    if name.is_empty() && is_directory == 1 && is_file == 0 {
        return conn.query_row(
            "SELECT COUNT(*) FROM files WHERE is_directory = 1",
            [],
            |r| r.get(0),
        );
    }

    conn.query_row(
        "SELECT COUNT(*)
         FROM files f
         JOIN file_names fn ON f.file_name_id = fn.id
         WHERE fn.name LIKE ?1
           AND (f.is_directory = ?2 OR f.is_file = ?3)",
        rusqlite::params![pattern, is_directory, is_file],
        |r| r.get(0),
    )
}

/// Search with SQL ORDER BY + LIMIT/OFFSET (does not load the full result set).
pub fn search_file_page(
    conn: &rusqlite::Connection,
    name: &str,
    target_kind: TargetKind,
    pattern_kind: PatternKind,
    order_kind: OrderKind,
    sort: &str,
    limit: u32,
    offset: u32,
) -> rusqlite::Result<Vec<FileEntry>> {
    let order = match order_kind {
        OrderKind::Asc => "ASC",
        OrderKind::Desc => "DESC",
    };
    let (pattern, is_directory, is_file) = pattern_and_types(name, target_kind, pattern_kind);
    let sort_col = sort_sql(sort);

    let sql = format!(
        "SELECT
            f.path, fn.name, f.size, f.modified, f.hash,
            f.is_directory, f.is_file, f.is_symlink, f.parent_path
         FROM files f
         JOIN file_names fn ON f.file_name_id = fn.id
         WHERE fn.name LIKE ?1
           AND (f.is_directory = ?2 OR f.is_file = ?3)
         ORDER BY {sort_col} {order}
         LIMIT ?4 OFFSET ?5"
    );

    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(rusqlite::params![
        pattern,
        is_directory,
        is_file,
        limit,
        offset,
    ])?;
    convert_from_rows(&mut rows)
}

/// Legacy full-result search (tests / CLI). Prefer `search_file_page`.
pub fn search_file(
    transaction: &rusqlite::Transaction,
    name: &str,
    target_kind: TargetKind,
    pattern_kind: PatternKind,
    order_kind: OrderKind,
) -> rusqlite::Result<Vec<FileEntry>> {
    search_file_page(
        transaction,
        name,
        target_kind,
        pattern_kind,
        order_kind,
        "name",
        u32::MAX,
        0,
    )
}
